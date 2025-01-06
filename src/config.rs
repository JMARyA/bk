use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Run a script before backup
    pub start_script: Option<String>,

    // Run a script after backup
    pub end_script: Option<String>,

    pub rsync: Option<Vec<RsyncConfig>>,
    pub borg: Option<Vec<BorgConfig>>,
    pub borg_check: Option<Vec<BorgCheckConfig>>,
    pub borg_prune: Option<Vec<BorgPruneConfig>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RsyncConfig {
    pub src: String,
    pub dest: String,
    pub exclude: Option<Vec<String>>,
    pub delete: Option<bool>,
    pub ensure_exists: Option<String>,
    pub cephfs_snap: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BorgConfig {
    pub repo: String,
    pub passphrase: Option<String>,
    pub src: Vec<String>,
    pub exclude: Option<Vec<String>>,
    pub exclude_if_present: Option<Vec<String>>,
    pub one_file_system: Option<bool>,
    pub ctime: Option<bool>,
    pub no_acls: Option<bool>,
    pub no_xattrs: Option<bool>,
    pub comment: Option<String>,
    pub compression: Option<String>,
    pub ensure_exists: Option<String>,
    pub cephfs_snap: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BorgCheckConfig {
    pub repo: String,
    pub verify_data: Option<bool>,
    pub repair: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BorgPruneConfig {
    pub repo: String,
    pub passphrase: String,
    pub keep_within: String,
    pub keep_last: Option<u32>,
    pub keep_secondly: Option<u32>,
    pub keep_minutely: Option<u32>,
    pub keep_hourly: Option<u32>,
    pub keep_daily: Option<u32>,
    pub keep_weekly: Option<u32>,
    pub keep_monthly: Option<u32>,
    pub keep_yearly: Option<u32>,
}
