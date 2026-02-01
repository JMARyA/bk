// INPUT

use facet::Facet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    cephfs_snap_create, cephfs_snap_remove, ensure_exists,
    restic::{bind_mount, umount},
};

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
