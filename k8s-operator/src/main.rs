use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;

use bk::config::ResticConfig;
use bk::config::ResticTarget;
use bk::config::S3Creds;
use bk::config::SSHOptions;
use futures::stream::StreamExt;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::batch::v1::CronJob;
use k8s_openapi::api::batch::v1::CronJobSpec;
use k8s_openapi::api::batch::v1::JobSpec;
use k8s_openapi::api::batch::v1::JobTemplateSpec;
use k8s_openapi::api::core::v1::Container;
use k8s_openapi::api::core::v1::KeyToPath;
use k8s_openapi::api::core::v1::PodSpec;
use k8s_openapi::api::core::v1::PodTemplateSpec;
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::api::core::v1::SecretVolumeSource;
use k8s_openapi::api::core::v1::Volume;
use k8s_openapi::api::core::v1::VolumeMount;
use kube::Resource;
use kube::ResourceExt;
use kube::api::DeleteParams;
use kube::api::ObjectMeta;
use kube::api::PostParams;
use kube::runtime::reflector::Lookup;
use kube::runtime::watcher::Config;
use kube::{Api, client::Client, runtime::Controller, runtime::controller::Action};
use tokio::time::Duration;

use crate::crd::ResticRepository;
use crate::crd::SSHConfig;
use crate::crd::SecretKeyRef;

pub mod crd;
mod finalizer;

#[tokio::main]
async fn main() {
    let kubernetes_client: Client = Client::try_default()
        .await
        .expect("Expected a valid KUBECONFIG.");

    let deployments: Api<Deployment> = Api::all(kubernetes_client.clone());
    let context: Arc<ContextData> = Arc::new(ContextData::new(kubernetes_client.clone()));

    Controller::new(deployments.clone(), Config::default())
        .run(reconcile, on_error, context)
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(echo_resource) => {
                    println!("Reconciliation successful. Resource: {:?}", echo_resource);
                }
                Err(reconciliation_err) => {
                    eprintln!("Reconciliation error: {:?}", reconciliation_err)
                }
            }
        })
        .await;
}

struct ContextData {
    client: Client,
}

impl ContextData {
    pub fn new(client: Client) -> Self {
        ContextData { client }
    }
}

enum EchoAction {
    Create,
    Delete,
    NoOp,
}

async fn reconcile(echo: Arc<Deployment>, context: Arc<ContextData>) -> Result<Action, Error> {
    let client: Client = context.client.clone(); // The `Client` is shared -> a clone from the reference is obtained

    let namespace: String = match Lookup::namespace(&*echo) {
        None => {
            // If there is no namespace to deploy to defined, reconciliation ends with an error immediately.
            return Err(Error::UserInputError(
                "Expected BackupDefinition resource to be namespaced. Can't deploy to an unknown namespace."
                    .to_owned(),
            ));
        }
        // If namespace is known, proceed. In a more advanced version of the operator, perhaps
        // the namespace could be checked for existence first.
        Some(namespace) => namespace.to_string(),
    };
    let name = echo.name_any(); // Name of the Echo resource is used to name the subresources as well.

    // Performs action as decided by the `determine_action` function.
    match determine_action(&echo) {
        EchoAction::Create => {
            let ns = echo.metadata.namespace.as_ref().unwrap();
            println!(
                "Found {} in {}",
                echo.metadata.name.as_ref().unwrap(),
                echo.metadata.namespace.as_ref().unwrap()
            );

            // Skip system namespaces
            if ns == "kube-system" {
                return Ok(Action::await_change());
            }

            if let Some(options) = BkOptions::parse(echo.metadata.annotations.as_ref().unwrap()) {
                let cjob = BackupCronJob::new(client.clone(), options, &echo).await;
                println!(
                    "Creating Backup Cron for {}",
                    echo.metadata.name.as_ref().unwrap()
                );

                add_finalizer!(
                    client,
                    Deployment,
                    &echo.name().unwrap(),
                    ns,
                    "bk.jmarya.me"
                );

                let cronjobs: Api<CronJob> = Api::namespaced(client.clone(), ns);
                match cronjobs.create(&PostParams::default(), &cjob).await {
                    Ok(_) => {
                        println!("Created CronJob for {}", name);
                    }
                    Err(kube::Error::Api(e)) if e.code == 409 => {
                        // Already exists, do an update instead
                        let current = cronjobs.get(&cjob.name().unwrap()).await?;
                        let mut updated = current.clone();

                        // You decide how much to update — possibly update .spec only
                        updated.spec = cjob.spec.clone();

                        println!("Updating CronJob {}", cjob.name().unwrap());
                        cronjobs
                            .replace(&cjob.name().unwrap(), &PostParams::default(), &updated)
                            .await?;
                    }
                    Err(e) => return Err(e.into()), // Other errors are real problems
                }
            }

            Ok(Action::requeue(Duration::from_secs(60)))
        }
        EchoAction::Delete => {
            let ns = echo.metadata.namespace.as_ref().unwrap();
            println!(
                "Deleting Backup Cron for {}",
                echo.metadata.name.as_ref().unwrap()
            );

            let cronjobs: Api<CronJob> = Api::namespaced(client.clone(), ns);
            cronjobs
                .delete(&format!("bk-backup-{name}"), &DeleteParams::default())
                .await;

            delete_finalizer!(
                client,
                Deployment,
                &echo.name().unwrap(),
                ns,
                "bk.jmarya.me"
            )
            .unwrap();

            Ok(Action::await_change())
        }
        // The resource is already in desired state, do nothing and re-check after 10 seconds
        EchoAction::NoOp => Ok(Action::requeue(Duration::from_secs(60))),
    }
}

/// Resources arrives into reconciliation queue in a certain state. This function looks at
/// the state of given `Echo` resource and decides which actions needs to be performed.
/// The finite set of possible actions is represented by the `EchoAction` enum.
///
/// # Arguments
/// - `echo`: A reference to `Echo` being reconciled to decide next action upon.
fn determine_action(echo: &Deployment) -> EchoAction {
    if echo.meta().deletion_timestamp.is_some() {
        EchoAction::Delete
    } else if echo
        .meta()
        .finalizers
        .as_ref()
        .map_or(true, |finalizers| finalizers.is_empty())
    {
        EchoAction::Create
    } else {
        EchoAction::NoOp
    }
}

/// Actions to be taken when a reconciliation fails - for whatever reason.
/// Prints out the error to `stderr` and requeues the resource for another reconciliation after
/// five seconds.
///
/// # Arguments
/// - `echo`: The erroneous resource.
/// - `error`: A reference to the `kube::Error` that occurred during reconciliation.
/// - `_context`: Unused argument. Context Data "injected" automatically by kube-rs.
fn on_error(echo: Arc<Deployment>, error: &Error, _context: Arc<ContextData>) -> Action {
    eprintln!("Reconciliation error:\n{:?}.\n{:?}", error, echo);
    Action::requeue(Duration::from_secs(5))
}

/// All errors possible to occur during reconciliation
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Any error originating from the `kube-rs` crate
    #[error("Kubernetes reported error: {source}")]
    KubeError {
        #[from]
        source: kube::Error,
    },
    /// Error in user input or Echo resource definition, typically missing fields.
    #[error("Invalid Echo CRD: {0}")]
    UserInputError(String),
}

pub struct BkOptions {
    pub repo: String,
    pub schedule: String,
}

impl BkOptions {
    pub fn parse(annotations: &BTreeMap<String, String>) -> Option<Self> {
        let annotations: serde_json::Value = serde_json::from_str(
            annotations.get("kubectl.kubernetes.io/last-applied-configuration")?,
        )
        .ok()?;
        let annotations = annotations
            .as_object()?
            .get("metadata")?
            .as_object()?
            .get("annotations")?
            .as_object()?;
        Some(Self {
            repo: annotations.get("bk/repository")?.as_str()?.to_string(),
            schedule: annotations.get("bk/schedule")?.as_str()?.to_string(),
        })
    }
}

pub struct BackupCronJob {}

impl BackupCronJob {
    pub async fn new(client: Client, options: BkOptions, deployment: &Deployment) -> CronJob {
        let ns = deployment.metadata.namespace.as_ref().unwrap().clone();
        let name = deployment.metadata.name.as_ref().unwrap().clone();

        // TODO : setup bk.conf

        let volumes = deployment
            .spec
            .as_ref()
            .unwrap()
            .template
            .spec
            .as_ref()
            .unwrap()
            .volumes
            .clone()
            .unwrap();
        let volume_mounts = deployment
            .spec
            .as_ref()
            .unwrap()
            .template
            .spec
            .as_ref()
            .unwrap()
            .containers
            .first()
            .as_ref()
            .unwrap()
            .volume_mounts
            .as_ref()
            .unwrap()
            .clone();

        let mut volumes: Vec<_> = volumes
            .into_iter()
            .filter(|x| x.host_path.is_some() || x.persistent_volume_claim.is_some())
            .collect();

        let mut volume_mounts: Vec<_> = volume_mounts
            .into_iter()
            .filter(|x| volumes.iter().any(|y| y.name == x.name))
            .collect();

        let mut paths = HashMap::new();

        for vol in &volume_mounts {
            paths.insert(
                vol.name.clone(),
                bk::config::LocalPath {
                    path: vol.mount_path.clone(),
                    ensure_exists: None,
                    cephfs_snap: None,
                    same_path: None,
                },
            );
        }

        let backends: Api<ResticRepository> = Api::namespaced(client.clone(), &ns);

        let backend = backends.get(&options.repo).await.unwrap();

        let secrets: Api<Secret> = Api::namespaced(client.clone(), &ns);

        let repo_key = get_secret(client.clone(), &ns, backend.spec.passphrase).await;

        let volume_tags: Vec<String> = volumes
            .iter()
            .map(|x| format!("volume_{}", x.name))
            .collect();

        if let Some(ssh) = &backend.spec.ssh {
            let (vol, mount) = mount_secret_file(
                "ssh-identity".to_string(),
                ssh.secret_key.secretName.clone(),
                ssh.secret_key.secretKey.clone(),
                "/etc/bk-ssh".to_string(),
            );
            volumes.push(vol);
            volume_mounts.push(mount);
        }

        // deployment: backup all volumes
        let conf = bk::config::Config {
            start_script: None,
            end_script: None,
            rsync: None,
            path: Some(paths.clone()),
            restic_target: Some({
                let mut h = HashMap::new();
                h.insert(
                    options.repo.clone(),
                    ResticTarget {
                        repo: backend.spec.endpoint,
                        s3: if let Some(s3) = backend.spec.s3 {
                            Some(S3Creds {
                                access_key: get_secret(client.clone(), &ns, s3.access_key).await,
                                secret_key: get_secret(client.clone(), &ns, s3.secret_key).await,
                            })
                        } else {
                            None
                        },
                        ssh: if let Some(ssh) = &backend.spec.ssh {
                            Some(SSHOptions {
                                port: None,
                                identity: format!("/etc/bk-ssh/{}", ssh.secret_key.secretKey),
                            })
                        } else {
                            None
                        },
                        passphrase: repo_key.to_string(),
                    },
                );
                h
            }),
            restic: Some(vec![ResticConfig {
                targets: vec![options.repo.clone()],
                src: paths.keys().map(|x| x.to_string()).collect::<Vec<_>>(),
                exclude: None,
                exclude_caches: None,
                reread: None,
                exclude_if_present: None,
                one_file_system: None,
                concurrency: None,
                tags: Some(volume_tags),
                compression: None,
                ntfy: None,
                quiet: None,
                host: Some(name.clone()),
            }]),
            ntfy: None,
        };

        let mut data: BTreeMap<String, String> = BTreeMap::new();
        data.insert("bk.toml".to_string(), toml::to_string(&conf).unwrap());

        let secret = Secret {
            metadata: kube::api::ObjectMeta {
                name: Some(format!("bk-backup-secret-{name}").to_string()),
                ..Default::default()
            },
            string_data: Some(data.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
            type_: Some("Opaque".to_string()),
            ..Default::default()
        };

        secrets.create(&PostParams::default(), &secret).await;

        // add config secret to volumes
        let (vol, mount) = mount_secret_file(
            "bk-config".to_string(),
            format!("bk-backup-secret-{name}"),
            "bk.toml".to_string(),
            "/etc/bk-config".to_string(),
        );
        volumes.push(vol);
        volume_mounts.push(mount);

        CronJob {
            metadata: ObjectMeta {
                name: format!("bk-backup-{name}").into(),
                namespace: ns.into(),
                ..Default::default()
            },
            spec: Some(CronJobSpec {
                concurrency_policy: Some("Forbid".to_string()),
                failed_jobs_history_limit: Some(3),
                job_template: JobTemplateSpec {
                    spec: Some(JobSpec {
                        template: PodTemplateSpec {
                            spec: Some(PodSpec {
                                containers: vec![Container {
                                    name: format!("backup-{name}"),
                                    image: Some("git.hydrar.de/jmarya/bk:latest".to_string()),
                                    command: Some(vec![
                                        "/usr/bin/bk".to_string(),
                                        "/etc/bk-config/bk.toml".to_string(),
                                    ]),
                                    volume_mounts: Some(volume_mounts),
                                    ..Default::default()
                                }],
                                restart_policy: Some("Never".to_string()),
                                volumes: Some(volumes),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                schedule: options.schedule,
                successful_jobs_history_limit: Some(5),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

pub async fn get_secret(client: Client, ns: &str, reference: SecretKeyRef) -> String {
    let secrets: Api<Secret> = Api::namespaced(client, &ns);
    let secret = secrets.get(&reference.secretName).await.unwrap();

    let secret_string_data = secret.data.unwrap();
    let value = secret_string_data.get(&reference.secretKey).unwrap();

    let value = String::from_utf8(value.0.clone()).unwrap();
    value
}

pub fn mount_secret_file(
    name: String,
    secret_name: String,
    secret_key: String,
    path: String,
) -> (Volume, VolumeMount) {
    (
        Volume {
            name: name.clone(),
            secret: SecretVolumeSource {
                secret_name: Some(secret_name),
                items: vec![KeyToPath {
                    key: secret_key.clone(),
                    path: secret_key,
                    mode: Some(0o600),
                    ..Default::default()
                }]
                .into(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        },
        VolumeMount {
            name: name,
            mount_path: path,
            read_only: true.into(),
            ..Default::default()
        },
    )
}
