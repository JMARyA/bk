// INPUT

use facet::Facet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yansi::{Color, Paint};

use crate::{
    cephfs_snap_create, cephfs_snap_remove, ensure_exists,
    restic::{bind_mount, umount},
};

fn is_root() -> bool {
    unsafe { libc::getuid() == 0 }
}

/// Local path input
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
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

/// S3 bucket input — mounted via geesefs (FUSE) so restic reads directly
/// without requiring a local copy of the bucket contents.
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct S3Input {
    /// S3 bucket name, optionally with a prefix (e.g. "mybucket" or "mybucket/prefix").
    pub bucket: String,

    /// Custom S3 endpoint URL for non-AWS providers (e.g. "https://s3.example.com").
    pub endpoint: Option<String>,

    /// AWS region (e.g. "us-east-1").
    pub region: Option<String>,

    /// Access key ID (inline).
    pub access_key: Option<String>,

    /// Read access key ID from a file.
    pub access_key_file: Option<String>,

    /// Secret access key (inline).
    #[facet(sensitive)]
    pub secret_key: Option<String>,

    /// Read secret access key from a file.
    pub secret_key_file: Option<String>,
}

impl S3Input {
    pub fn access_key(&self) -> Option<String> {
        self.access_key.clone().or_else(|| {
            self.access_key_file
                .as_ref()
                .and_then(|f| std::fs::read_to_string(f).ok())
                .map(|s| s.trim().to_string())
        })
    }

    pub fn secret_key(&self) -> Option<String> {
        self.secret_key.clone().or_else(|| {
            self.secret_key_file
                .as_ref()
                .and_then(|f| std::fs::read_to_string(f).ok())
                .map(|s| s.trim().to_string())
        })
    }
}

pub struct S3InputRef {
    pub conf: S3Input,
    pub mount_path: String,
    pub mounted: bool,
}

impl S3InputRef {
    pub fn new(key: &str, conf: S3Input) -> Self {
        let safe_key = key.replace(['/', '\\', ' ', ':'], "_");
        let mount_path = if is_root() {
            format!("/var/cache/bk/s3/{safe_key}")
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            format!("{home}/.cache/bk/s3/{safe_key}")
        };
        Self { conf, mount_path, mounted: false }
    }

    pub fn get_target_path(&mut self) -> String {
        std::fs::create_dir_all(&self.mount_path).unwrap_or_else(|e| {
            log::error!("failed to create S3 mount point {}: {e}", self.mount_path);
            std::process::exit(1);
        });

        log::info!(
            "Mounting S3 bucket {} at {}",
            self.conf.bucket.paint(Color::Yellow),
            self.mount_path.paint(Color::Cyan),
        );

        let mut cmd = std::process::Command::new("geesefs");

        if let Some(endpoint) = &self.conf.endpoint {
            cmd.arg("--endpoint").arg(endpoint);
        }

        if let Some(key) = self.conf.access_key() {
            cmd.env("AWS_ACCESS_KEY_ID", key);
        }
        if let Some(secret) = self.conf.secret_key() {
            cmd.env("AWS_SECRET_ACCESS_KEY", secret);
        }
        if let Some(region) = &self.conf.region {
            cmd.env("AWS_REGION", region);
        }

        cmd.arg("-o").arg("ro").arg(&self.conf.bucket).arg(&self.mount_path);

        let status = cmd.status().unwrap_or_else(|e| {
            log::error!("failed to run geesefs: {e}");
            std::process::exit(1);
        });

        if !status.success() {
            log::error!(
                "geesefs failed to mount bucket {}",
                self.conf.bucket
            );
            let _ = std::fs::remove_dir(&self.mount_path);
            std::process::exit(1);
        }

        self.mounted = true;
        self.mount_path.clone()
    }
}

impl Drop for S3InputRef {
    fn drop(&mut self) {
        if self.mounted {
            log::info!("Unmounting S3 bucket at {}", self.mount_path);
            umount(&self.mount_path);
        }
    }
}

/// Unified input reference — either a local path or an S3 bucket mount.
pub enum InputRef {
    Local(LocalPathRef),
    S3(S3InputRef),
}

impl InputRef {
    pub fn get_target_path(&mut self) -> String {
        match self {
            Self::Local(r) => r.get_target_path(),
            Self::S3(r) => r.get_target_path(),
        }
    }
}
