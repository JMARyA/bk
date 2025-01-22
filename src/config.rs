use serde::Deserialize;

/// Configuration structure for the backup system.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Optional script to run before starting the backup process.
    pub start_script: Option<String>,

    /// Optional script to run after completing the backup process.
    pub end_script: Option<String>,

    /// Configuration for rsync jobs.
    pub rsync: Option<Vec<RsyncConfig>>,

    /// Configuration for Borg backup jobs.
    pub borg: Option<Vec<BorgConfig>>,

    /// Configuration for Borg check jobs to verify repository integrity.
    pub borg_check: Option<Vec<BorgCheckConfig>>,

    /// Configuration for Borg prune jobs to manage repository snapshots.
    pub borg_prune: Option<Vec<BorgPruneConfig>>,
}

/// Configuration for an individual rsync job.
#[derive(Debug, Clone, Deserialize)]
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

/// Configuration for an individual Borg backup job.
#[derive(Debug, Clone, Deserialize)]
pub struct BorgConfig {
    /// Borg repository path.
    pub repo: String,

    /// Optional passphrase for the repository.
    pub passphrase: Option<String>,

    /// List of source paths to include in the backup.
    pub src: Vec<String>,

    /// List of patterns to exclude from the backup.
    pub exclude: Option<Vec<String>>,

    /// List of marker files; directories containing these will be excluded.
    pub exclude_if_present: Option<Vec<String>>,

    /// Whether to limit the backup to a single filesystem.
    pub one_file_system: Option<bool>,

    /// Whether to backup ctime
    pub ctime: Option<bool>,

    /// Disable backup of ACLs (Access Control Lists).
    pub no_acls: Option<bool>,

    /// Disable backup of extended attributes.
    pub no_xattrs: Option<bool>,

    /// Optional comment to associate with the backup.
    pub comment: Option<String>,

    /// Compression mode to use for the backup.
    pub compression: Option<String>,

    /// Ensure a specific directory exists before running the backup.
    pub ensure_exists: Option<String>,

    /// Create CephFS snapshots before the backup.
    pub cephfs_snap: Option<bool>,

    /// Bind mount to consistent path
    pub same_path: Option<bool>,
}

/// Configuration for a Borg repository integrity check job.
#[derive(Debug, Clone, Deserialize)]
pub struct BorgCheckConfig {
    /// Borg repository path.
    pub repo: String,

    /// Whether to verify the repository's data integrity.
    pub verify_data: Option<bool>,

    /// Whether to attempt repairs on detected issues.
    pub repair: Option<bool>,
}

/// Configuration for a Borg repository pruning job.
#[derive(Debug, Clone, Deserialize)]
pub struct BorgPruneConfig {
    /// Borg repository path.
    pub repo: String,

    /// Passphrase for accessing the repository.
    pub passphrase: String,

    /// Retain all archives within this time period.
    pub keep_within: String,

    /// Retain the last `n` archives.
    pub keep_last: Option<u32>,

    /// Retain the last `n` secondly archives.
    pub keep_secondly: Option<u32>,

    /// Retain the last `n` minutely archives.
    pub keep_minutely: Option<u32>,

    /// Retain the last `n` hourly archives.
    pub keep_hourly: Option<u32>,

    /// Retain the last `n` daily archives.
    pub keep_daily: Option<u32>,

    /// Retain the last `n` weekly archives.
    pub keep_weekly: Option<u32>,

    /// Retain the last `n` monthly archives.
    pub keep_monthly: Option<u32>,

    /// Retain the last `n` yearly archives.
    pub keep_yearly: Option<u32>,
}
