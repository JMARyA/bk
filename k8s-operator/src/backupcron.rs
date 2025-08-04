use std::collections::BTreeMap;
use std::collections::HashMap;

use bk::config::ResticConfig;
use bk::config::ResticTarget;
use bk::config::S3Creds;
use bk::config::SSHOptions;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::batch::v1::CronJob;
use k8s_openapi::api::batch::v1::CronJobSpec;
use k8s_openapi::api::batch::v1::JobSpec;
use k8s_openapi::api::batch::v1::JobTemplateSpec;
use k8s_openapi::api::core::v1::Container;
use k8s_openapi::api::core::v1::PodSpec;
use k8s_openapi::api::core::v1::PodTemplateSpec;
use k8s_openapi::api::core::v1::Volume;
use k8s_openapi::api::core::v1::VolumeMount;
use kube::api::ObjectMeta;
use kube::{Api, client::Client};

use crate::crd::ResticRepository;
use crate::secrets::create_secret;
use crate::secrets::get_secret;
use crate::secrets::mount_secret_file;

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
    /// Extract the volumes from a `Deployment`
    pub fn get_vols_of_deployment(deployment: &Deployment) -> (Vec<Volume>, Vec<VolumeMount>) {
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

        let volumes: Vec<_> = volumes
            .into_iter()
            // only backup hostPath or PersistentVolumeClaim
            .filter(|x| x.host_path.is_some() || x.persistent_volume_claim.is_some())
            .collect();

        let volume_mounts: Vec<_> = volume_mounts
            .into_iter()
            .filter(|x| volumes.iter().any(|y| y.name == x.name))
            .collect();

        (volumes, volume_mounts)
    }

    pub async fn get_remote_config(
        client: Client,
        ns: &str,
        repo_name: &str,
        volumes: &mut Vec<Volume>,
        volume_mounts: &mut Vec<VolumeMount>,
    ) -> HashMap<String, ResticTarget> {
        let backends: Api<ResticRepository> = Api::namespaced(client.clone(), &ns);

        let backend = backends.get(&repo_name).await.unwrap();

        let repo_key = get_secret(client.clone(), &ns, backend.spec.passphrase).await;

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

        let mut h = HashMap::new();

        h.insert(
            repo_name.to_string(),
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
    }

    pub async fn new(client: Client, options: BkOptions, deployment: &Deployment) -> CronJob {
        let ns = deployment.metadata.namespace.as_ref().unwrap().clone();
        let name = deployment.metadata.name.as_ref().unwrap().clone();

        let (mut volumes, mut volume_mounts) = Self::get_vols_of_deployment(deployment);

        // setup bk.conf
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

        let volume_tags: Vec<String> = volumes
            .iter()
            .map(|x| format!("volume_{}", x.name))
            .collect();

        // deployment: backup all volumes
        let conf = bk::config::Config {
            start_script: None,
            end_script: None,
            rsync: None,
            path: Some(paths.clone()),
            restic_target: Some(
                Self::get_remote_config(
                    client.clone(),
                    &ns,
                    &options.repo,
                    &mut volumes,
                    &mut volume_mounts,
                )
                .await,
            ),
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

        create_secret(
            client.clone(),
            &ns,
            {
                let mut h = HashMap::new();
                h.insert("bk.toml".to_string(), toml::to_string(&conf).unwrap())
                    .unwrap();
                h
            },
            Self::cronjob_secret_name(&name),
        )
        .await
        .unwrap();

        // add config secret to volumes
        let (vol, mount) = mount_secret_file(
            "bk-config".to_string(),
            Self::cronjob_secret_name(&name),
            "bk.toml".to_string(),
            "/etc/bk-config".to_string(),
        );
        volumes.push(vol);
        volume_mounts.push(mount);

        return Self::create_cronjob(&name, &ns, volume_mounts, volumes, options);
    }

    pub fn cronjob_name(name: &str) -> String {
        format!("bk-backup-{name}")
    }

    pub fn cronjob_secret_name(name: &str) -> String {
        format!("bk-backup-secret-{name}")
    }

    pub fn create_cronjob(
        name: &str,
        ns: &str,
        volume_mounts: Vec<VolumeMount>,
        volumes: Vec<Volume>,
        options: BkOptions,
    ) -> CronJob {
        CronJob {
            metadata: ObjectMeta {
                name: Some(Self::cronjob_name(name)),
                namespace: Some(ns.to_string()),
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
