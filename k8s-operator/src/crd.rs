use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Restic Repository
#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "bk.jmarya.me",
    version = "v1",
    kind = "ResticRepository",
    plural = "restic-repositories",
    derive = "PartialEq",
    namespaced
)]
pub struct ResticRepositorySpec {
    /// The endpoint of the repository. Can be either a path, s3 or sftp.
    pub endpoint: String,
    /// S3 credentials
    pub s3: Option<S3Config>,
    /// SSH credentials
    pub ssh: Option<SSHConfig>,
    /// Repository password
    pub passphrase: SecretKeyRef,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
pub struct S3Config {
    pub access_key: SecretKeyRef,
    pub secret_key: SecretKeyRef,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
pub struct SSHConfig {
    pub secret_key: SecretKeyRef,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[allow(non_snake_case)]
pub struct SecretKeyRef {
    pub secretName: String,
    pub secretKey: String,
}

/// Resource for backing up the filesystem of a node
#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "bk.jmarya.me",
    version = "v1",
    kind = "NodeBackup",
    plural = "node-backups",
    derive = "PartialEq",
    namespaced
)]
pub struct NodeBackupSpec {
    /// Target repository
    pub repository: String,
    /// Source paths to backup
    pub paths: Vec<String>,
    /// Cron schedule for the backup
    pub schedule: String,
    /// Which node the backup runs on
    pub node: String,
    /// Quiet mode
    pub quiet: Option<bool>,
    /// Create a cephFS snapshot before backup
    pub cephfs_snap: Option<bool>,
}
