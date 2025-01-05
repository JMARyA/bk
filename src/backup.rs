use yansi::{Color, Paint};

use crate::{
    config::{Config, RsyncConfig},
    run_command,
};

pub fn ensure_exists(dir: &str) {
    let exists = std::fs::exists(dir).unwrap_or_default();
    let entries = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .collect::<Vec<_>>();

    if !exists || entries.len() == 0 {
        println!(
            "{} {}",
            "Error:".paint(Color::Red),
            "Directory {dir} does not exists"
        );
        std::process::exit(1);
    }
}

pub fn run_backup_rsync(conf: &RsyncConfig) {
    println!(
        "Running backup for {} -> {}",
        conf.src.paint(Color::Blue),
        conf.dest.paint(Color::Yellow)
    );

    if let Some(dir) = &conf.ensure_exists {
        ensure_exists(&dir);
    }

    let mut cmd = vec!["rsync", "-avzhruP"];

    if conf.delete.unwrap_or_default() {
        cmd.push("--delete");
    }

    if let Some(exclude) = &conf.exclude {
        for e in exclude {
            cmd.extend(&["--exclude", e.as_str()]);
        }
    }

    cmd.push(&conf.src);
    cmd.push(&conf.dest);

    run_command(&cmd);
}

pub fn run_backup(conf: Config) {
    if let Some(script) = &conf.start_script {
        run_command(&["sh", script.as_str()]);
    }

    for rsync in &conf.rsync.unwrap_or_default() {
        run_backup_rsync(rsync);
    }

    for borg in &conf.borg.unwrap_or_default() {
        // TODO : Implement
    }

    if let Some(script) = &conf.end_script {
        run_command(&["sh", script.as_str()]);
    }
}
