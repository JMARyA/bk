use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    pub endpoint: String,
    pub s3: Option<S3Config>,
    pub ssh: Option<SSHConfig>,
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
    pub repository: String,
    pub paths: Vec<String>,
    pub schedule: String,
    pub node: String,
}
