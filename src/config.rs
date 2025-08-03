use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    backup::{cephfs_snap_create, cephfs_snap_remove, ensure_exists},
    notify::ntfy,
    restic::{bind_mount, umount},
};

/// Configuration structure for the backup system.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Optional script to run before starting the backup process.
    pub start_script: Option<String>,

    /// Optional script to run after completing the backup process.
    pub end_script: Option<String>,

    // CDRs
    /// Local path inputs
    pub path: Option<HashMap<String, LocalPath>>,

    /// Configuration for rsync jobs.
    pub rsync: Option<Vec<RsyncConfig>>,

    /// Restic targets
    pub restic_target: Option<HashMap<String, ResticTarget>>,

    /// Configuration for Borg backup jobs.
    pub restic: Option<Vec<ResticConfig>>,

    /// Ntfy targets
    pub ntfy: Option<HashMap<String, NtfyTarget>>,
}

/// Configuration for an individual rsync job.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RsyncConfig {
    /// Source path for rsync.
    pub src: String,

    /// Destination path for rsync.
    pub dest: String,

    /// List of patterns to exclude from synchronization.
    pub exclude: Option<Vec<String>>,

    /// Whether to delete files at the destination that are not in the source.
    pub delete: Option<bool>,

    /// Ensure a specific directory exists before running the rsync job.
    pub ensure_exists: Option<String>,

    /// Create CephFS snapshot before the rsync job.
    pub cephfs_snap: Option<bool>,
}

/// Configuration for a restic target.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResticTarget {
    /// Restic repository URL.
    pub repo: String,

    /// S3 Credentials
    pub s3: Option<S3Creds>,

    /// SSH Options
    pub ssh: Option<SSHOptions>,

    /// Optional passphrase for the repository.
    pub passphrase: String,
}

/// S3 Credentials
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct S3Creds {
    pub access_key: String,
    pub secret_key: String,
}

/// SSH Options
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SSHOptions {
    pub port: Option<u16>,
    pub identity: String,
}

/// Configuration for an individual restic backup job.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResticConfig {
    /// Notifications
    pub ntfy: Option<Vec<String>>,

    /// Restic targets
    pub targets: Vec<String>,

    /// List of source paths to include in the backup.
    pub src: Vec<String>,

    /// List of patterns to exclude from the backup.
    pub exclude: Option<Vec<String>>,

    /// Cache directories marked with CACHEDIR.TAG will be excluded
    pub exclude_caches: Option<bool>,

    /// Reread all files even if unchanged
    pub reread: Option<bool>,

    /// List of marker files; directories containing these will be excluded.
    pub exclude_if_present: Option<Vec<String>>,

    /// Whether to limit the backup to a single filesystem.
    pub one_file_system: Option<bool>,

    /// Read concurrency
    pub concurrency: Option<u64>,

    /// Optional comment to associate with the backup.
    pub tags: Option<Vec<String>>,

    /// Compression mode to use for the backup.
    pub compression: Option<String>,

    // Quiet mode
    pub quiet: Option<bool>,

    /// Host override
    pub host: Option<String>
}

// INPUT

/// Local path input
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LocalPath {
    /// The local path
    pub path: String,

    /// Ensure a specific directory exists before running the backup.
    pub ensure_exists: Option<bool>,

    /// Create CephFS snapshots before the backup.
    pub cephfs_snap: Option<bool>,

    /// Bind mount to consistent path after snapshot creation
    pub same_path: Option<bool>,
}

pub struct LocalPathRef {
    pub conf: LocalPath,
    pub cephfs_snap_name: Option<String>,
    pub bind_mount_path: Option<String>,
}

impl LocalPathRef {
    pub fn from(conf: LocalPath) -> Self {
        Self {
            conf,
            cephfs_snap_name: None,
            bind_mount_path: None,
        }
    }

    pub fn get_target_path(&mut self) -> String {
        if self.conf.ensure_exists.unwrap_or(true) {
            ensure_exists(&self.conf.path);
        }

        if self.conf.cephfs_snap.unwrap_or_default() {
            let (final_dir, snap_name) = cephfs_snap_create(&self.conf.path);
            self.cephfs_snap_name = Some(snap_name);

            if self.conf.same_path.unwrap_or_default() {
                let name = self.conf.path.replace("/", "_");
                log::info!("Creating consistent path /bk/{}", name);
                std::fs::create_dir_all(&format!("/bk/{name}")).unwrap();
                let bind_mount_path = format!("/bk/{name}");
                bind_mount(&final_dir, &bind_mount_path);
                self.bind_mount_path = Some(bind_mount_path.clone());
                return bind_mount_path;
            } else {
                return final_dir;
            }
        }

        self.conf.path.clone()
    }

    pub fn cleanup(&self) {
        if let Some(bmount) = &self.bind_mount_path {
            log::info!("Cleaning up mount {}", bmount);
            umount(&bmount);
        }

        if let Some(snap) = &self.cephfs_snap_name {
            log::info!(
                "Cleaning up snapshot {}",
                format!("{}@{}", self.conf.path, snap)
            );
            cephfs_snap_remove(&self.conf.path, &snap);
        }
    }
}

impl Drop for LocalPathRef {
    fn drop(&mut self) {
        self.cleanup();
    }
}

// Notification

/// Ntfy configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NtfyTarget {
    pub ntfy: Option<NtfyConfiguration>,
}

impl NtfyTarget {
    pub fn send_notification(&self, msg: &str) {
        if let Some(ntfy_conf) = &self.ntfy {
            ntfy(
                &ntfy_conf.host,
                &ntfy_conf.topic,
                ntfy_conf.auth.clone().map(|x| (x.user, x.pass)),
                msg,
            )
            .unwrap();
        }
    }
}

/// Ntfy configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NtfyConfiguration {
    pub host: String,
    pub topic: String,
    pub auth: Option<NtfyAuth>,
}

/// Ntfy configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NtfyAuth {
    pub user: String,
    pub pass: String,
}
