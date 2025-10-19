use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    backup::{cephfs_snap_create, cephfs_snap_remove, ensure_exists},
    notify::ntfy,
    restic::{bind_mount, umount},
};

/// Configuration structure for the backup system.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct Config {
    /// Optional script to run before starting the backup process.
    pub start_script: Option<String>,

    /// Optional script to run after completing the backup process.
    pub end_script: Option<String>,

    /// Optional Max Jitter Delay in seconds. Randomized wait time to evenly distribute backups if started via exact cron for example
    pub delay: Option<u64>,

    // CDRs
    /// Local path inputs
    pub path: Option<HashMap<String, LocalPath>>,

    /// Configuration for rsync jobs.
    pub rsync: Option<Vec<RsyncConfig>>,

    /// Restic targets
    pub restic_target: Option<HashMap<String, ResticTarget>>,

    /// Configuration for restic backup jobs.
    pub restic: Option<Vec<ResticConfig>>,

    /// Configuration for restic forget jobs
    pub restic_forget: Option<Vec<ResticForget>>,

    /// Ntfy targets
    pub ntfy: Option<HashMap<String, NtfyTarget>>,
}

impl Config {
    pub fn from_path(path: &str) -> Self {
        toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }
}

/// Configuration for an individual rsync job.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
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
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ResticTarget {
    /// Restic repository URL.
    pub repo: String,

    /// S3 Credentials
    pub s3: Option<S3Creds>,

    /// SSH Options
    pub ssh: Option<SSHOptions>,

    /// Optional passphrase for the repository.
    pub passphrase: Option<String>,

    /// Read passphrase from file
    pub passphrase_file: Option<String>,
}

/// S3 Credentials
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct S3Creds {
    pub access_key: String,
    pub secret_key: String,
}

/// SSH Options
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SSHOptions {
    pub port: Option<u16>,
    pub identity: String,
}

/// Configuration for an individual restic backup job.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
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
    pub host: Option<String>,
}
/// Configuration for an individual restic forget job.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ResticForget {
    /// Notifications (e.g. ntfy topics to notify after job)
    pub ntfy: Option<Vec<String>>,

    /// Restic repository targets
    pub targets: Vec<String>,

    /// keep the last n snapshots (use "unlimited" to keep all)
    pub keep_last: Option<u64>,

    /// keep the last n hourly snapshots
    pub keep_hourly: Option<u64>,

    /// keep the last n daily snapshots
    pub keep_daily: Option<u64>,

    /// keep the last n weekly snapshots
    pub keep_weekly: Option<u64>,

    /// keep the last n monthly snapshots
    pub keep_monthly: Option<u64>,

    /// keep the last n yearly snapshots
    pub keep_yearly: Option<u64>,

    /// keep snapshots newer than this duration (e.g. "1y5m7d2h")
    pub keep_within: Option<u64>,

    /// keep hourly snapshots newer than this duration
    pub keep_within_hourly: Option<u64>,

    /// keep daily snapshots newer than this duration
    pub keep_within_daily: Option<u64>,

    /// keep weekly snapshots newer than this duration
    pub keep_within_weekly: Option<u64>,

    /// keep monthly snapshots newer than this duration
    pub keep_within_monthly: Option<u64>,

    /// keep yearly snapshots newer than this duration
    pub keep_within_yearly: Option<u64>,

    /// keep snapshots with these tags
    pub keep_tag: Option<Vec<String>>,

    /// allow deleting all snapshots of a snapshot group
    pub unsafe_allow_remove_all: Option<bool>,

    /// only consider snapshots for this host
    pub host: Option<Vec<String>>,

    /// only consider snapshots with these tags
    pub tag: Option<Vec<String>>,

    /// only consider snapshots including these paths
    pub path: Option<Vec<String>>,

    /// use compact output format
    pub compact: Option<bool>,

    /// group snapshots by host, paths, and/or tags (disable grouping with "")
    pub group_by: Option<String>,

    /// automatically run 'prune' if snapshots were removed
    pub prune: Option<bool>,

    /// tolerate this amount of unused data (default "5%")
    pub max_unused: Option<String>,

    /// stop after repacking this much data
    pub max_repack_size: Option<String>,

    /// only repack packs which are cacheable
    pub repack_cacheable_only: Option<bool>,

    /// repack pack files below 80% of target pack size
    pub repack_small: Option<bool>,

    /// repack all uncompressed data
    pub repack_uncompressed: Option<bool>,

    /// repack packfiles below this size
    pub repack_smaller_than: Option<String>,
}

// INPUT

/// Local path input
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
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
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct NtfyTarget {
    pub ntfy: Option<NtfyConfiguration>,
}

impl NtfyTarget {
    pub fn send_notification(&self, msg: &str) {
        if let Some(ntfy_conf) = &self.ntfy {
            ntfy(
                &ntfy_conf.host,
                &ntfy_conf.topic,
                ntfy_conf.auth.clone().map(|x| x.auth()),
                msg,
            )
            .unwrap();
        }
    }
}

/// Ntfy configuration
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct NtfyConfiguration {
    pub host: String,
    pub topic: String,
    pub auth: Option<NtfyAuth>,
}

/// Ntfy configuration
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct NtfyAuth {
    pub user: String,
    pub pass: Option<String>,
    pub pass_file: Option<String>,
}

impl NtfyAuth {
    pub fn auth(&self) -> (String, String) {
        let pass = if let Some(pass) = &self.pass {
            Some(pass.clone())
        } else if let Some(pass) = &self.pass_file {
            Some(std::fs::read_to_string(pass).expect("unable to read ntfy passfile"))
        } else {
            None
        };

        (
            self.user.clone(),
            pass.expect("neither pass nor passfile provided"),
        )
    }
}
