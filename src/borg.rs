use yansi::{Color, Paint};

use crate::{
    backup::nowtime,
    config::{BorgCheckConfig, BorgConfig, BorgPruneConfig},
    run_command,
};

pub fn init_repo(path: &str) {
    run_command(&["borg", "init", "--encryption=repokey-blake2", path], None);
}

pub fn create_archive(conf: &BorgConfig) {
    let archive_name = format!(
        "BK_{}_{}_{}",
        std::fs::read_to_string("/etc/hostname")
            .map(|x| x.trim().to_string())
            .unwrap_or(String::from("UNKNOWN")),
        conf.src
            .iter()
            .map(|x| x.trim_start_matches("/").replace("/", "-"))
            .collect::<Vec<_>>()
            .join("+++"),
        nowtime()
    );

    println!(
        "--> Running backup for {}",
        conf.src.join(",").paint(Color::Yellow),
    );
    println!(
        "--> Creating borg archive {}",
        format!("{}::{archive_name}", conf.repo).paint(Color::Yellow),
    );

    let mut cmd = vec!["borg", "create", "--stats", "--list", "--progress"];

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

    if !conf.ctime.unwrap_or(true) {
        cmd.push("--noctime");
    }

    if conf.no_acls.unwrap_or_default() {
        cmd.push("--noacls");
    }

    if conf.no_xattrs.unwrap_or_default() {
        cmd.push("--noxattrs");
    }

    if let Some(comment) = &conf.comment {
        cmd.push("--comment");
        cmd.push(comment);
    }

    let zstd10 = "zstd,10".to_string();
    let comp = conf.compression.as_ref().unwrap_or(&zstd10);
    cmd.push("--compression");
    cmd.push(comp);

    let repo = format!("{}::{}", conf.repo, archive_name);

    cmd.push(&repo);

    for path in &conf.src {
        cmd.push(path);
    }

    run_command(&cmd, conf.passphrase.clone());
}

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
