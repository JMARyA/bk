use std::collections::BTreeMap;
use std::collections::HashMap;

use bk::config::Config;
use bk::config::ResticConfig;
use bk::config::ResticTarget;
use bk::config::S3Creds;
use bk::config::SSHOptions;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::batch::v1::CronJob;
use k8s_openapi::api::batch::v1::CronJobSpec;
use k8s_openapi::api::batch::v1::JobSpec;
use k8s_openapi::api::batch::v1::JobTemplateSpec;
use k8s_openapi::api::core::v1::Capabilities;
use k8s_openapi::api::core::v1::Container;
use k8s_openapi::api::core::v1::HostPathVolumeSource;
use k8s_openapi::api::core::v1::PodSecurityContext;
use k8s_openapi::api::core::v1::PodSpec;
use k8s_openapi::api::core::v1::PodTemplateSpec;
use k8s_openapi::api::core::v1::SeccompProfile;
use k8s_openapi::api::core::v1::SecurityContext;
use k8s_openapi::api::core::v1::Volume;
use k8s_openapi::api::core::v1::VolumeMount;
use kube::api::ObjectMeta;
use kube::{Api, client::Client};

use crate::crd::NodeBackup;
use crate::crd::ResticRepository;
use crate::secrets::get_secret;
use crate::secrets::mount_secret_file;

/// Options for bk cron
pub struct BkOptions {
    /// repository
    pub repo: String,
    /// cron schedule
    pub schedule: String,
    /// comma seperated list of volumes to exclude
    pub exclude: Option<Vec<String>>,
    /// cephfs snap + same path
    pub cephfs_snap: Option<Vec<String>>,
}

impl BkOptions {
    /// Parse `BkOptions` from the annotations of a resource
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

        let excludes = annotations.get("bk/exclude").and_then(|x| {
            x.as_str().map(|x| {
                x.split(",")
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
            })
        });

        let snaps = annotations.get("bk/cephfs_snap").and_then(|x| {
            x.as_str().map(|x| {
                x.split(",")
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
            })
        });

        Some(Self {
            repo: annotations.get("bk/repository")?.as_str()?.to_string(),
            schedule: annotations.get("bk/schedule")?.as_str()?.to_string(),
            exclude: excludes,
            cephfs_snap: snaps,
        })
    }
}

pub struct BackupCronJob {}

impl BackupCronJob {
    /// Transform a filesystem path into a valid name
    pub fn path_to_name(path: &str) -> String {
        path.replace("/", "-")
            .trim_start_matches("-")
            .to_lowercase()
            .to_string()
    }

    /// Transform `NodeBackup` into `Volume`s and `VolumeMount`s
    ///
    /// This goes through the paths in `NodeBackup` and generates respective Volume entries via hostPath.
    pub fn get_vols_of_nodebackup(nodebackup: &NodeBackup) -> (Vec<Volume>, Vec<VolumeMount>) {
        let mut vol = Vec::new();
        let mut volm = Vec::new();

        for path in &nodebackup.spec.paths {
            vol.push(Volume {
                name: Self::path_to_name(&path),
                host_path: Some(HostPathVolumeSource {
                    path: path.clone(),
                    ..Default::default()
                }),
                ..Default::default()
            });
            volm.push(VolumeMount {
                name: Self::path_to_name(&path),
                mount_path: format!("/host{path}"),
                ..Default::default()
            });
        }

        (vol, volm)
    }

    /// Extract the volumes from a `Deployment`
    ///
    /// It will only extract `hostPath` and `PersistentVolumeClaim` Volumes.
    pub fn get_vols_of_deployment(deployment: &Deployment) -> (Vec<Volume>, Vec<VolumeMount>) {
        Self::extract_vols_of_podspec(
            deployment
                .spec
                .as_ref()
                .unwrap()
                .template
                .spec
                .as_ref()
                .unwrap(),
        )
    }

    pub fn get_vols_of_statefulset(statefulset: &StatefulSet) -> (Vec<Volume>, Vec<VolumeMount>) {
        Self::extract_vols_of_podspec(
            statefulset
                .spec
                .as_ref()
                .unwrap()
                .template
                .spec
                .as_ref()
                .unwrap(),
        )
    }

    pub fn extract_vols_of_podspec(pod: &PodSpec) -> (Vec<Volume>, Vec<VolumeMount>) {
        // TODO : handle multiple containers in pod; currently only first one is sourced

        let volumes = pod.volumes.clone().unwrap();
        let volume_mounts = pod
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

    /// Get the remote config (restic target config) from the environment.
    ///
    /// This will try to load a `ResticRepository` named `repo_name` in the namespace `ns`.
    /// Additionally it modifies the volumes to include an SSH key secret mount if needed.
    pub async fn get_remote_config(
        client: Client,
        ns: &str,
        repo_name: &str,
        volumes: &mut Vec<Volume>,
        volume_mounts: &mut Vec<VolumeMount>,
    ) -> Result<HashMap<String, ResticTarget>, crate::Error> {
        let backends: Api<ResticRepository> = Api::namespaced(client.clone(), &ns);

        let backend = backends.get(&repo_name).await.map_err(|_| {
            crate::Error::UserInputError(format!(
                "Restic repository {repo_name} could not be sourced. Does it exist?"
            ))
        })?;

        let repo_key = get_secret(client.clone(), &ns, backend.spec.passphrase)
            .await
            .map_err(|_| {
                crate::Error::UserInputError(format!(
                    "Could not get passphrase secret for repository {repo_name}"
                ))
            })?;

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
                        access_key: get_secret(client.clone(), &ns, s3.access_key)
                            .await
                            .map_err(|_| {
                                crate::Error::UserInputError(format!(
                                    "Could not get S3 access key secret for repository {repo_name}"
                                ))
                            })?,
                        secret_key: get_secret(client.clone(), &ns, s3.secret_key)
                            .await
                            .map_err(|_| {
                                crate::Error::UserInputError(format!(
                                    "Could not get S3 secret key secret for repository {repo_name}"
                                ))
                            })?,
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
        Ok(h)
    }

    pub fn build_bk_conf(
        volume_mounts: Vec<VolumeMount>,
        targets: HashMap<String, ResticTarget>,
        target: String,
        host: String,
        excludes: Option<Vec<String>>,
        cephfs_snap: Option<Vec<String>>,
    ) -> Config {
        let mut paths = HashMap::new();

        for vol in &volume_mounts {
            if vol.name == "ssh-identity" {
                continue;
            }

            if let Some(excludes) = &excludes {
                if excludes.contains(&vol.name) {
                    continue;
                }
            }

            let is_snap = cephfs_snap
                .as_ref()
                .map(|x| x.contains(&vol.name))
                .unwrap_or(false);

            paths.insert(
                vol.name.clone(),
                bk::config::LocalPath {
                    path: vol.mount_path.clone(),
                    ensure_exists: None,
                    cephfs_snap: if is_snap { Some(true) } else { None },
                    same_path: if is_snap { Some(true) } else { None },
                },
            );
        }

        let volume_tags: Vec<String> = volume_mounts
            .iter()
            .filter(|x| {
                if x.name == "ssh-identity" {
                    return false;
                }

                if let Some(excludes) = &excludes {
                    if excludes.contains(&x.name) {
                        return false;
                    }
                }

                return true;
            })
            .map(|x| format!("volume_{}", x.name))
            .collect();

        bk::config::Config {
            start_script: None,
            end_script: None,
            // 30 min random delay to even out resource utilitization if schedules are the same
            delay: Some(1800),
            rsync: None,
            path: Some(paths.clone()),
            restic_target: Some(targets),
            restic: Some(vec![ResticConfig {
                targets: vec![target],
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
                host: Some(host),
            }]),
            ntfy: None,
        }
    }

    pub fn cronjob_name(name: &str) -> String {
        format!("bk-backup-{name}")
    }

    pub fn cronjob_secret_name(name: &str) -> String {
        format!("bk-backup-secret-{name}")
    }

    pub fn node_cronjob_name(name: &str) -> String {
        format!("bk-nodebackup-{name}")
    }

    pub fn node_cronjob_secret_name(name: &str) -> String {
        format!("bk-nodebackup-secret-{name}")
    }

    pub fn create_cronjob(
        cron_name: String,
        name: &str,
        ns: &str,
        volume_mounts: Vec<VolumeMount>,
        volumes: Vec<Volume>,
        options: BkOptions,
    ) -> CronJob {
        CronJob {
            metadata: ObjectMeta {
                name: Some(cron_name),
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
                                    security_context: Some(SecurityContext {
                                        capabilities: Some(Capabilities {
                                            add: vec!["SYS_ADMIN".to_string()].into(),
                                            ..Default::default()
                                        }),
                                        seccomp_profile: Some(SeccompProfile {
                                            type_: "Unconfined".to_string(),
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    }),
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
