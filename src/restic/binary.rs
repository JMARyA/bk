use cmdbind::{CommandOutput, wrap_binary};

use crate::config::ResticConfig;

#[derive(Debug, Default, serde::Serialize)]
pub struct ResticInitArgs {
    pub option: Vec<String>,
    pub json: bool,
    pub repo: String,
}

pub fn restic_init_validator(out: &CommandOutput) -> bool {
    cmdbind::validators::status_not_one_of(&[1], out)
}

wrap_binary!(
    ResticInitCommand,
    "restic",
    ResticInitArgs,
    restic_init_validator,
    "init"
);

#[derive(derive_builder::Builder)]
#[builder(setter(into), default)]
#[derive(Debug, Default, serde::Serialize)]
pub struct ResticBackupCommandArgs {
    pub exclude: Option<Vec<String>>,
    #[serde(rename = "exclude-if-present")]
    pub exclude_if_present: Option<Vec<String>>,
    #[serde(rename = "one-file-system")]
    pub one_file_system: bool,
    #[serde(rename = "read-concurrency")]
    pub read_concurrency: u64,
    pub tag: Vec<String>,
    #[serde(rename = "force")]
    pub reread: bool,
    #[serde(rename = "exclude-caches")]
    pub exclude_caches: bool,
    pub option: Vec<String>,
    #[serde(rename = "dry-run")]
    pub dry_run: bool,
    pub compression: String,
    pub quiet: bool,
    pub json: bool,
    pub host: Option<String>,
    pub repo: String,
    pub positional_0_dir: Vec<String>,
}

impl ResticBackupCommandArgs {
    pub fn from_config(conf: ResticConfig) -> Self {
        Self {
            exclude: conf.exclude,
            exclude_if_present: conf.exclude_if_present,
            one_file_system: conf.one_file_system.unwrap_or_default(),
            read_concurrency: conf.concurrency.unwrap_or(2),
            tag: conf.tags.unwrap_or_default(),
            reread: conf.reread.unwrap_or_default(),
            exclude_caches: conf.exclude_caches.unwrap_or_default(),
            option: Vec::new(),
            dry_run: false,
            compression: conf.compression.unwrap_or("auto".to_string()),
            quiet: conf.quiet.unwrap_or_default(),
            json: conf.quiet.unwrap_or_default(),
            host: conf.host,
            repo: String::new(),
            positional_0_dir: Vec::new(),
        }
    }
}

pub fn restic_backup_validator(out: &CommandOutput) -> bool {
    cmdbind::validators::status_not_one_of(&[1, 3, 10, 11, 12], out)
}

wrap_binary!(
    ResticBackupCommand,
    "restic",
    ResticBackupCommandArgs,
    restic_backup_validator,
    "backup"
);

#[derive(derive_builder::Builder)]
#[builder(setter(into), default)]
#[derive(Debug, Default, serde::Serialize)]
pub struct ResticTagArgs {
    pub add: Vec<String>,

    pub remove: Vec<String>,

    pub set: Vec<String>,

    pub option: Vec<String>,

    pub quiet: bool,
    pub json: bool,
    pub repo: String,
    pub positional_0_snapshot: Vec<String>,
}

wrap_binary!(
    ResticTagCommand,
    "restic",
    ResticTagArgs,
    restic_backup_validator,
    "tag"
);

#[derive(Debug, serde::Serialize)]
pub struct ResticSnapshotsArgs {
    pub positional_snapshot_id: Vec<String>,

    pub compact: bool,
    pub group_by: Option<String>,
    pub host: Option<String>,
    pub latest: Option<i64>,
    pub path: Option<String>,
    pub tag: Option<String>,

    pub json: bool,
    pub no_lock: bool,
    pub repo: String,
}

wrap_binary!(
    ResticSnapshotsCommand,
    "restic",
    ResticSnapshotsArgs,
    restic_backup_validator,
    "snapshots"
);
