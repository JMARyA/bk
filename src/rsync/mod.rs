use facet::Facet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yansi::{Color, Paint};

use crate::{cephfs_snap_create, cephfs_snap_remove, ensure_exists, run_command};

/// Configuration for an individual rsync job.
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
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

impl RsyncConfig {
    pub fn run_backup(&self, dry: bool) {
        println!(
            "--> Running backup for {} -> {}",
            self.src.paint(Color::Yellow),
            self.dest.paint(Color::Yellow)
        );

        if let Some(dir) = &self.ensure_exists {
            ensure_exists(dir);
        }

        let mut cmd = vec!["rsync", "-avzhruP"];

        if self.delete.unwrap_or_default() {
            cmd.push("--delete");
        }

        if dry {
            cmd.push("--dry-run")
        }

        if let Some(exclude) = &self.exclude {
            for e in exclude {
                cmd.extend(&["--exclude", e.as_str()]);
            }
        }

        if self.cephfs_snap.unwrap_or_default() {
            let (snap_dir, snap_name) = cephfs_snap_create(&self.src);
            cmd.push(&snap_dir);
            cmd.push(&self.dest);
            run_command(&cmd, None);
            cephfs_snap_remove(&self.src, &snap_name);
        } else {
            cmd.push(&self.src);
            cmd.push(&self.dest);
            run_command(&cmd, None);
        }
    }
}
