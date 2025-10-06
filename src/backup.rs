use rand::Rng;
use yansi::{Color, Paint};

use crate::{
    args::RunCommand,
    config::{Config, RsyncConfig},
    restic, run_command,
};

pub fn ensure_exists(dir: &str) {
    let exists = std::fs::exists(dir).unwrap_or_default();
    let entries = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .collect::<Vec<_>>();

    if !exists || entries.is_empty() {
        println!(
            "{} Directory {dir} does not exists",
            "Error:".paint(Color::Red),
        );
        std::process::exit(1);
    }
}

pub fn run_backup_rsync(conf: &RsyncConfig) {
    println!(
        "--> Running backup for {} -> {}",
        conf.src.paint(Color::Yellow),
        conf.dest.paint(Color::Yellow)
    );

    if let Some(dir) = &conf.ensure_exists {
        ensure_exists(dir);
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

    if conf.cephfs_snap.unwrap_or_default() {
        let (snap_dir, snap_name) = cephfs_snap_create(&conf.src);
        cmd.push(&snap_dir);
        cmd.push(&conf.dest);
        run_command(&cmd, None);
        cephfs_snap_remove(&conf.src, &snap_name);
    } else {
        cmd.push(&conf.src);
        cmd.push(&conf.dest);
        run_command(&cmd, None);
    }
}

pub fn run_backup(args: RunCommand) -> i32 {
    let conf = Config::from_path(&args.config);
    let mut state = 0;

    if let Some(delay) = conf.delay {
        let wait = rand::random_range(0..delay);
        log::info!("Delaying backup for {wait} seconds...");
        std::thread::sleep(std::time::Duration::from_secs(wait));
    }

    if let Some(script) = &conf.start_script {
        run_command(&["sh", script.as_str()], None);
    }

    for rsync in &conf.rsync.unwrap_or_default() {
        run_backup_rsync(rsync);
    }

    for restic in &conf.restic.unwrap_or_default() {
        if !args.path.iter().any(|x| restic.src.contains(x)) && !args.path.is_empty() {
            log::info!(
                "Skipping restic operation due to path filter: want {:?}, got {:?}",
                args.path,
                restic.src
            );
            continue;
        }

        let res = restic::create_archive(
            restic,
            conf.path.clone().unwrap_or_default(),
            conf.restic_target.clone().unwrap_or_default(),
        );

        for (target, res) in res {
            let notify_provider = conf.ntfy.clone().unwrap_or_default();

            if let Err(e) = res {
                log::error!("Backup to target {target} failed: {e}");
                state = 1;

                for ntfy_key in restic.ntfy.clone().unwrap_or_default() {
                    let ntfy_opt = notify_provider.get(&ntfy_key).unwrap();
                    ntfy_opt.send_notification(&format!(
                        "🚨 Backup failed for {} to {}: {e}",
                        restic.src.join(", "),
                        target
                    ));
                }
            } else {
                log::info!("Backup successfull for {target}");

                for ntfy_key in restic.ntfy.clone().unwrap_or_default() {
                    let ntfy_opt = notify_provider.get(&ntfy_key).unwrap();
                    ntfy_opt.send_notification(&format!(
                        "✅ Backup successful for {:?} to {}",
                        restic.src, target
                    ));
                }
            }
        }
    }

    if let Some(script) = &conf.end_script {
        run_command(&["sh", script.as_str()], None);
    }

    return state;
}

pub fn now() -> String {
    chrono::Utc::now().format("%Y_%m_%d").to_string()
}

pub fn nowtime() -> String {
    chrono::Utc::now().format("%Y_%m_%d-%H_%M").to_string()
}

pub fn cephfs_snap_create(dir: &str) -> (String, String) {
    let path = std::path::Path::new(dir);
    let now = now();
    let snap_name = format!("SNAP_{now}");
    let snap_dir = path.join(".snap").join(&snap_name);

    log::info!("Creating snapshot {} on {}", snap_name, dir);
    if std::fs::create_dir(&snap_dir).is_err() {
        if !std::fs::exists(&snap_dir).unwrap() {
            log::error!("{} Could not create snapshot", "Error:".paint(Color::Red));
            std::process::exit(1);
        }
    }

    (format!("{}/", snap_dir.to_str().unwrap()), snap_name)
}

pub fn cephfs_snap_remove(dir: &str, snap: &str) {
    let path = std::path::Path::new(dir);
    let snap_dir = path.join(".snap").join(snap);

    log::info!("Removing snapshot {} on {}", snap, dir);
    std::fs::remove_dir(snap_dir).unwrap()
}

pub fn cephfs_snap_remove_dir(dir: &str) {
    let path = std::path::Path::new(dir);

    log::info!("Removing snapshot {}", path.to_str().unwrap());
    std::fs::remove_dir(path).unwrap()
}
