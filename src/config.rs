use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Run a script before backup
    pub start_script: Option<String>,

    // Run a script after backup
    pub end_script: Option<String>,

    pub rsync: Option<Vec<RsyncConfig>>,
    pub borg: Option<Vec<BorgConfig>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RsyncConfig {
    pub src: String,
    pub dest: String,
    pub exclude: Option<Vec<String>>,
    pub delete: Option<bool>,
    pub ensure_exists: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BorgConfig {
    pub repo: String,
    pub passphrase: Option<String>,
    pub src: Vec<String>,
}
