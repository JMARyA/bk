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
) -> HashMap<String, Result<(), ResticError>> {
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

    let mut targets_results = HashMap::new();

    for repo in targets {
        log::info!(
            "Running backup for {} on {}",
            conf.src.join(",").paint(Color::Yellow),
            repo.repo.paint(Color::Yellow)
        );

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

        if conf.quiet.unwrap_or_default() {
            cmd.push("--quiet");
        }

        if let Some(host) = &conf.host {
            cmd.push("--host");
            cmd.push(host);
        }

        cmd.push("-r");
        cmd.push(&repo.repo);

        cmd.extend(dirs.iter().map(|x| x.as_str()));

        let mut env = Vec::new();

        env.push(("RESTIC_PASSWORD".to_string(), repo.passphrase.clone()));

        if let Some(s3) = &repo.s3 {
            env.push(("AWS_ACCESS_KEY_ID".to_string(), s3.access_key.clone()));
            env.push(("AWS_SECRET_ACCESS_KEY".to_string(), s3.secret_key.clone()));
        }

        if let Some(ssh) = &repo.ssh {
            env.push(("RESTIC_SFTP_COMMAND".to_string(), 
            format!("ssh -i {} {} -o StrictHostKeyChecking=no %u@%h -s sftp", ssh.identity, if let Some(p) = ssh.port { format!("-p {p}") } else { String::new() })));
        }

        let res = run_command(&cmd, Some(env));

        if res.2 == 0 {
            targets_results.insert(repo.repo.clone(), Ok(()));
        } else {
            let err = ResticError::from_code(res.2).unwrap();
            targets_results.insert(repo.repo.clone(), Err(err));
        }
    }

    targets_results
}

pub enum ResticError {
    /// Return Code 1 - fatal error (no snapshot created)
    Fatal,
    /// Return Code 3 - some source data could not be read (incomplete snapshot created)
    Incomplete,
    /// Return Code 10 - repository does not exist
    RepositoryUnavailable,
    /// Return Code 11 - repository is already locked
    RepositoryLocked,
    /// Return Code 12 - incorrect password
    IncorrectPassword,
}

impl std::fmt::Display for ResticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ResticError::Fatal => "Fatal Error (no snapshot created)",
            ResticError::Incomplete => {
                "some source data could not be read (incomplete snapshot created)"
            }
            ResticError::RepositoryUnavailable => "repository does not exist",
            ResticError::RepositoryLocked => "repository is already locked",
            ResticError::IncorrectPassword => "incorrect password",
        })
    }
}

impl ResticError {
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            1 => Some(Self::Fatal),
            3 => Some(Self::Incomplete),
            10 => Some(Self::RepositoryUnavailable),
            11 => Some(Self::RepositoryLocked),
            12 => Some(Self::IncorrectPassword),
            _ => None,
        }
    }
}
