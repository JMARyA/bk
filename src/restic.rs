use yansi::{Color, Paint};

use crate::{
    backup::{cephfs_snap_create, cephfs_snap_remove_dir, ensure_exists, nowtime},
    config::{BorgCheckConfig, BorgConfig, BorgPruneConfig, ResticConfig},
    run_command,
};

pub fn bind_mount(src: &str, dst: &str) {
    run_command(&["mount", "--bind", src, dst], None);
}

pub fn umount(mount: &str) {
    run_command(&["umount", mount], None);
}

pub fn create_archive(conf: &ResticConfig) {
    if let Some(dir) = &conf.ensure_exists {
        ensure_exists(dir);
    }

    println!(
        "--> Running backup for {}",
        conf.src.join(",").paint(Color::Yellow),
    );
    println!("--> Creating restic archive");

    let mut cmd = vec!["restic", "backup"];

    let empty = Vec::new();
    for ex in conf.exclude.as_ref().unwrap_or(&empty) {
        cmd.push("--exclude");
        cmd.push(ex);
    }

    for ex in conf.exclude_if_present.as_ref().unwrap_or(&empty) {
        cmd.push("--exclude-if-present");
        cmd.push(ex);
    }

    if conf.one_file_system.unwrap_or_default() {
        cmd.push("--one-file-system");
    }

    let c = conf.concurrency.unwrap_or(2).to_string();
    cmd.push("--read-concurrency");
    cmd.push(&c);

    let tags = conf.tags.clone().unwrap_or_default();

    for t in &tags {
        cmd.push("--tag");
        cmd.push(&t);
    }

    if conf.reread.unwrap_or_default() {
        cmd.push("--force");
    }

    if conf.exclude_caches.unwrap_or_default() {
        cmd.push("--exclude-caches");
    }

    let zstd10 = "auto".to_string();
    let comp = conf.compression.as_ref().unwrap_or(&zstd10);
    cmd.push("--compression");
    cmd.push(comp);

    cmd.push("-r");
    cmd.push(&conf.repo);

    let mut snaps = Vec::new();

    if conf.cephfs_snap.unwrap_or_default() {
        for path in &conf.src {
            let snap = cephfs_snap_create(&path);
            snaps.push((snap.0, snap.1, path));
        }
    }

    let mut dirs = if snaps.is_empty() {
        conf.src
            .clone()
            .into_iter()
            .map(|x| (x.clone(), x))
            .collect()
    } else {
        snaps
            .iter()
            .map(|x| (x.0.clone(), x.2.clone()))
            .collect::<Vec<_>>()
    };

    let mut mounts = Vec::new();
    if conf.same_path.unwrap_or_default() {
        for (path, orig) in &dirs {
            let name = orig.replace("/", "_");
            println!("--> Creating consistent path /bk/{}", name);
            std::fs::create_dir_all(&format!("/bk/{name}")).unwrap();
            bind_mount(path, &format!("/bk/{name}"));
            mounts.push((format!("/bk/{name}"), path.clone()));
        }

        dirs = mounts.clone();
    }

    cmd.extend(dirs.iter().map(|x| x.0.as_str()));

    run_command(
        &cmd,
        Some(vec![(
            "RESTIC_PASSWORD".to_string(),
            conf.passphrase.clone(),
        )]),
    );

    for cleanup in &snaps {
        cephfs_snap_remove_dir(&cleanup.0);
        println!("--> Cleaning up snap {}", cleanup.0);
    }

    for (cleanup, _) in &mounts {
        println!("--> Cleaning up mount {}", cleanup);
        umount(&cleanup);
    }
}

// TODO : todo

/*
pub fn prune_archive(conf: &BorgPruneConfig) {
    println!("--> Pruning borg repo {}", conf.repo.paint(Color::Yellow),);

    let mut cmd = vec!["borg", "prune", "--stats", "--list"];

    cmd.push(&conf.keep_within);

    let binding = conf
    .keep_last
    .as_ref()
    .map(|x| format!("--keep-last={x}"))
    .unwrap_or_default();
cmd.push(&binding);
let binding = conf
.keep_secondly
.as_ref()
.map(|x| format!("--keep-secondly={x}"))
.unwrap_or_default();
cmd.push(&binding);
let binding = conf
.keep_minutely
.as_ref()
.map(|x| format!("--keep-minutely={x}"))
.unwrap_or_default();
cmd.push(&binding);
let binding = conf
.keep_hourly
.as_ref()
        .map(|x| format!("--keep-hourly={x}"))
        .unwrap_or_default();
    cmd.push(&binding);
    let binding = conf
    .keep_daily
    .as_ref()
    .map(|x| format!("--keep-daily={x}"))
    .unwrap_or_default();
cmd.push(&binding);
let binding = conf
.keep_weekly
.as_ref()
.map(|x| format!("--keep-weekly={x}"))
.unwrap_or_default();
cmd.push(&binding);
let binding = conf
.keep_monthly
.as_ref()
.map(|x| format!("--keep-monthly={x}"))
.unwrap_or_default();
cmd.push(&binding);
let binding = conf
.keep_yearly
.as_ref()
.map(|x| format!("--keep-yearly={x}"))
.unwrap_or_default();
cmd.push(&binding);

run_command(&cmd, Some(conf.passphrase.clone()));

let cmd = vec!["borg", "compact", &conf.repo];
run_command(&cmd, Some(conf.passphrase.clone()));
}

pub fn check_archive(conf: &BorgCheckConfig) {
    println!("--> Checking borg repo {}", conf.repo.paint(Color::Yellow),);

    let mut cmd = vec!["borg", "check"];

    if conf.verify_data.unwrap_or_default() {
        cmd.push("--verify-data");
    } else {
        cmd.push("--repository-only");
        cmd.push("--archives-only");
    }

    if conf.repair.unwrap_or_default() {
        cmd.push("--repair");
    }

    cmd.push(&conf.repo);

    run_command(&cmd, None);
}
*/
