use std::collections::HashMap;

use yansi::{Color, Paint};

use crate::{
    config::{LocalPath, LocalPathRef, ResticConfig, ResticTarget},
    run_command,
};

pub fn bind_mount(src: &str, dst: &str) {
    run_command(&["mount", "--bind", src, dst], None);
}

pub fn umount(mount: &str) {
    run_command(&["umount", mount], None);
}

pub fn create_archive(
    conf: &ResticConfig,
    path_provider: HashMap<String, LocalPath>,
    target_provider: HashMap<String, ResticTarget>,
) {
    let mut paths: Vec<_> = conf
        .src
        .iter()
        .map(|x| {
            if let Some(pp) = path_provider.get(x) {
                let pp = LocalPathRef::from(pp.clone());
                return pp;
            } else {
                log::error!("Unknown path provider {x}");
                std::process::exit(1);
            }
        })
        .collect();

    let mut dirs = Vec::new();

    for path in &mut paths {
        dirs.push(path.get_target_path());
    }

    let targets: Vec<_> = conf
        .targets
        .iter()
        .map(|x| {
            if let Some(pp) = target_provider.get(x) {
                return pp;
            } else {
                log::error!("Unknown restic provider {x}");
                std::process::exit(1);
            }
        })
        .collect();

    for repo in targets {
        println!(
            "--> Running backup for {} on {}",
            conf.src.join(",").paint(Color::Yellow),
            repo.repo.paint(Color::Yellow)
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
        cmd.push(&repo.repo);

        cmd.extend(dirs.iter().map(|x| x.as_str()));

        run_command(
            &cmd,
            Some(vec![(
                "RESTIC_PASSWORD".to_string(),
                repo.passphrase.clone(),
            )]),
        );
    }
}
