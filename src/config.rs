use std::collections::HashMap;

use facet::Facet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    cephfs_snap_create, cephfs_snap_remove, ensure_exists,
    input::LocalPath,
    notify::{NtfyTarget, ntfy},
    restic::{bind_mount, find_password, umount},
    rsync::RsyncConfig,
};

/// Configuration structure for the backup system.
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct Config {
    /// Optional script to run before starting the backup process.
    pub start_script: Option<String>,

    /// Optional script to run after completing the backup process.
    pub end_script: Option<String>,

    /// Optional Max Jitter Delay in seconds. Randomized wait time to evenly distribute backups if started via exact cron for example
    pub delay: Option<u64>,

    /// Home Server
    pub home: Option<String>,

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
        facet_toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }
}

/// Configuration for a restic target.
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct ResticTarget {
    /// Restic repository URL.
    pub repo: String,

    /// S3 Credentials
    pub s3: Option<S3Creds>,

    /// SSH Options
    pub ssh: Option<SSHOptions>,

    /// Optional passphrase for the repository.
    #[facet(sensitive)]
    pub passphrase: Option<String>,

    /// Read passphrase from file
    pub passphrase_file: Option<String>,
}

/// S3 Credentials
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct S3Creds {
    pub access_key: Option<String>,
    #[facet(sensitive)]
    pub secret_key: Option<String>,
    pub access_key_file: Option<String>,
    pub secret_key_file: Option<String>,
}

impl S3Creds {
    pub fn access_key(&self) -> Option<String> {
        find_password(&self.access_key, &self.access_key_file)
    }

    pub fn secret_key(&self) -> Option<String> {
        find_password(&self.secret_key, &self.secret_key_file)
    }
}

/// SSH Options
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct SSHOptions {
    pub port: Option<u16>,
    pub identity: String,
}

/// Configuration for an individual restic backup job.
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct ResticConfig {
    /// Notifications
    pub ntfy: Option<Vec<String>>,

    /// Restic targets
    pub targets: Vec<String>,

    #[facet(flatten)]
    pub options: ResticBackupConfig,
}

#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct ResticBackupConfig {
    /// The target
    #[facet(default)]
    pub target: String,

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
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct ResticForget {
    /// Notifications (e.g. ntfy topics to notify after job)
    pub ntfy: Option<Vec<String>>,

    /// Restic repository targets
    pub targets: Vec<String>,

    #[serde(flatten)]
    pub args: ResticForgetArgs,
}

#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct ResticForgetArgs {
    #[serde(rename = "dry-run")]
    pub dry_run: bool,
    pub repo: Option<String>,
    pub option: Vec<String>,

    /// keep the last n snapshots (use "unlimited" to keep all)
    #[serde(rename = "keep-last")]
    pub keep_last: Option<u64>,

    /// keep the last n hourly snapshots
    #[serde(rename = "keep-hourly")]
    pub keep_hourly: Option<u64>,

    /// keep the last n daily snapshots
    #[serde(rename = "keep-daily")]
    pub keep_daily: Option<u64>,

    /// keep the last n weekly snapshots
    #[serde(rename = "keep-weekly")]
    pub keep_weekly: Option<u64>,

    /// keep the last n monthly snapshots
    #[serde(rename = "keep-monthly")]
    pub keep_monthly: Option<u64>,

    /// keep the last n yearly snapshots
    #[serde(rename = "keep-yearly")]
    pub keep_yearly: Option<u64>,

    /// keep snapshots newer than this duration (e.g. "1y5m7d2h")
    #[serde(rename = "keep-within")]
    pub keep_within: Option<u64>,

    /// keep hourly snapshots newer than this duration
    #[serde(rename = "keep-within-hourly")]
    pub keep_within_hourly: Option<u64>,

    /// keep daily snapshots newer than this duration
    #[serde(rename = "keep-within-daily")]
    pub keep_within_daily: Option<u64>,

    /// keep weekly snapshots newer than this duration
    #[serde(rename = "keep-within-weekly")]
    pub keep_within_weekly: Option<u64>,

    /// keep monthly snapshots newer than this duration
    #[serde(rename = "keep-within-monthly")]
    pub keep_within_monthly: Option<u64>,

    /// keep yearly snapshots newer than this duration
    #[serde(rename = "keep-within-yearly")]
    pub keep_within_yearly: Option<u64>,

    /// keep snapshots with these tags
    #[serde(rename = "keep-tag")]
    pub keep_tag: Option<Vec<String>>,

    /// allow deleting all snapshots of a snapshot group
    #[serde(rename = "unsafe-allow-remove-all")]
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
    #[serde(rename = "group-by")]
    pub group_by: Option<String>,

    /// automatically run 'prune' if snapshots were removed
    pub prune: Option<bool>,

    /// tolerate this amount of unused data (default "5%")
    #[serde(rename = "max-unused")]
    pub max_unused: Option<String>,

    /// stop after repacking this much data
    #[serde(rename = "max-repack-size")]
    pub max_repack_size: Option<String>,

    /// only repack packs which are cacheable
    #[serde(rename = "repack-cacheable-only")]
    pub repack_cacheable_only: Option<bool>,

    /// repack pack files below 80% of target pack size
    #[serde(rename = "repack-small")]
    pub repack_small: Option<bool>,

    /// repack all uncompressed data
    #[serde(rename = "repack-uncompressed")]
    pub repack_uncompressed: Option<bool>,

    /// repack packfiles below this size
    #[serde(rename = "repack-smaller-than")]
    pub repack_smaller_than: Option<String>,
}
