use std::{cell::RefCell, collections::HashMap};

use yansi::{Color, Paint};

use crate::{
    config::{LocalPath, LocalPathRef, ResticConfig, ResticForget, ResticTarget},
    run_command,
};

pub fn bind_mount(src: &str, dst: &str) {
    run_command(&["mount", "--bind", src, dst], None);
}

pub fn umount(mount: &str) {
    run_command(&["umount", mount], None);
}

const NO_S3_CREDS: &str = "no s3 credentials provided";

pub fn create_archive(
    conf: &ResticConfig,
    path_provider: HashMap<String, LocalPath>,
    target_provider: HashMap<String, ResticTarget>,
    dry: bool,
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

        if dry {
            cmd.push("--dry-run");
        }

        let zstd10 = "auto".to_string();
        let comp = conf.compression.as_ref().unwrap_or(&zstd10);
        cmd.push("--compression");
        cmd.push(comp);

        if conf.quiet.unwrap_or_default() {
            cmd.push("--quiet");
            cmd.push("--json");
        }

        if let Some(host) = &conf.host {
            cmd.push("--host");
            cmd.push(host);
        }

        cmd.push("-r");
        cmd.push(&repo.repo);

        cmd.extend(dirs.iter().map(|x| x.as_str()));

        let mut env = Vec::new();

        if let Some(passphrase) = &repo.passphrase {
            env.push(("RESTIC_PASSWORD".to_string(), passphrase.clone()));
        } else if let Some(pass_file) = &repo.passphrase_file {
            let passphrase =
                std::fs::read_to_string(pass_file).expect("Could not read passphrase file");
            env.push(("RESTIC_PASSWORD".to_string(), passphrase));
        } else {
            log::error!(
                "Neither passphrase nor passphrase file provided for {}",
                repo.repo
            );
            targets_results.insert(repo.repo.clone(), Err(ResticError::Fatal));
        }

        if let Some(s3) = &repo.s3 {
            env.push((
                "AWS_ACCESS_KEY_ID".to_string(),
                s3.access_key().expect(NO_S3_CREDS).clone(),
            ));
            env.push((
                "AWS_SECRET_ACCESS_KEY".to_string(),
                s3.secret_key().expect(NO_S3_CREDS).clone(),
            ));
        }

        let mut ssh_opt = None;

        if let Some(ssh) = &repo.ssh {
            let remote = repo.repo.trim_start_matches("sftp:");
            let hostpart = remote.split(':').collect::<Vec<_>>();
            let hostpart = hostpart.first().unwrap();
            let (user, host) = hostpart.split_once('@').unwrap();
            let ssh_cmd = format!(
                "ssh -i {} {} -o StrictHostKeyChecking=no {user}@{host} -s sftp",
                ssh.identity,
                if let Some(p) = ssh.port {
                    format!("-p {p}")
                } else {
                    String::new()
                }
            );
            ssh_opt = Some(format!("sftp.command={ssh_cmd}"));
        }

        if let Some(ssh_opt) = &ssh_opt {
            cmd.push("-o");
            cmd.push(ssh_opt);
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

pub fn forget_archive(
    conf: &ResticForget,
    target_provider: HashMap<String, ResticTarget>,
    dry: bool,
) -> HashMap<String, Result<(), ResticError>> {
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
            "Running backup forget for {}",
            repo.repo.paint(Color::Yellow)
        );

        let mut cmd = vec!["restic", "forget"];

        if conf.compact.unwrap_or_default() {
            cmd.push("--compact");
        }

        let istr = IntStrings::new();

        if let Some(val) = &conf.keep_last {
            cmd.push("--keep-last");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_hourly {
            cmd.push("--keep-hourly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_daily {
            cmd.push("--keep-daily");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_weekly {
            cmd.push("--keep-weekly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_monthly {
            cmd.push("--keep-monthly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_yearly {
            cmd.push("--keep-yearly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_within {
            cmd.push("--keep-within");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_within_hourly {
            cmd.push("--keep-within-hourly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_within_daily {
            cmd.push("--keep-within-daily");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_within_weekly {
            cmd.push("--keep-within-weekly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_within_monthly {
            cmd.push("--keep-within-monthly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_within_yearly {
            cmd.push("--keep-within-yearly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_daily {
            cmd.push("--keep-daily");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_weekly {
            cmd.push("--keep-weekly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_monthly {
            cmd.push("--keep-monthly");
            cmd.push(istr.format(*val));
        }

        if let Some(val) = &conf.keep_tag {
            for val in val {
                cmd.push("--keep-tag");
                cmd.push(val.as_str());
            }
        }

        if conf.unsafe_allow_remove_all.unwrap_or_default() {
            cmd.push("--unsafe-allow-remove-all");
        }

        if let Some(val) = &conf.host {
            for val in val {
                cmd.push("--host");
                cmd.push(val.as_str());
            }
        }

        if let Some(val) = &conf.path {
            for val in val {
                cmd.push("--path");
                cmd.push(val.as_str());
            }
        }

        if let Some(val) = &conf.group_by {
            cmd.push("--group-by");
            cmd.push(val.as_str());
        }

        if conf.prune.unwrap_or_default() {
            cmd.push("--prune");
        }

        if let Some(val) = &conf.max_unused {
            cmd.push("--max-unused");
            cmd.push(val.as_str());
        }

        if let Some(val) = &conf.max_repack_size {
            cmd.push("--max-repack-size");
            cmd.push(val.as_str());
        }

        if conf.repack_cacheable_only.unwrap_or_default() {
            cmd.push("--repack-cacheable-only");
        }

        if conf.repack_small.unwrap_or_default() {
            cmd.push("--repack-small");
        }

        if conf.repack_uncompressed.unwrap_or_default() {
            cmd.push("--repack-uncompressed");
        }

        if let Some(val) = &conf.repack_smaller_than {
            cmd.push("--repack-smaller-than");
            cmd.push(val.as_str());
        }

        if dry {
            cmd.push("--dry-run");
        }

        cmd.push("-r");
        cmd.push(&repo.repo);

        let mut env = Vec::new();

        let passphrase = find_password(&repo.passphrase, &repo.passphrase_file);
        match passphrase {
            Some(passphrase) => {
                env.push(("RESTIC_PASSWORD".to_string(), passphrase.clone()));
            }
            None => {
                log::error!(
                    "Neither passphrase nor passphrase file provided for {}",
                    repo.repo
                );
                targets_results.insert(repo.repo.clone(), Err(ResticError::Fatal));
            }
        }

        if let Some(s3) = &repo.s3 {
            env.push((
                "AWS_ACCESS_KEY_ID".to_string(),
                s3.access_key().expect(NO_S3_CREDS).clone(),
            ));
            env.push((
                "AWS_SECRET_ACCESS_KEY".to_string(),
                s3.secret_key().expect(NO_S3_CREDS).clone(),
            ));
        }

        let mut ssh_opt = None;

        if let Some(ssh) = &repo.ssh {
            let remote = repo.repo.trim_start_matches("sftp:");
            let hostpart = remote.split(':').collect::<Vec<_>>();
            let hostpart = hostpart.first().unwrap();
            let (user, host) = hostpart.split_once('@').unwrap();
            let ssh_cmd = format!(
                "ssh -i {} {} -o StrictHostKeyChecking=no {user}@{host} -s sftp",
                ssh.identity,
                if let Some(p) = ssh.port {
                    format!("-p {p}")
                } else {
                    String::new()
                }
            );
            ssh_opt = Some(format!("sftp.command={ssh_cmd}"));
        }

        if let Some(ssh_opt) = &ssh_opt {
            cmd.push("-o");
            cmd.push(ssh_opt);
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

pub fn find_password(password: &Option<String>, pass_file: &Option<String>) -> Option<String> {
    match password {
        Some(_) => {
            return password.clone();
        }
        None => {
            if let Some(pass_file) = pass_file {
                let passphrase =
                    std::fs::read_to_string(pass_file).expect("Could not read passphrase file");
                return Some(passphrase);
            }
        }
    }

    None
}

pub struct IntStrings {
    // Keep the Box<str> so we can drop them safely
    bufs: RefCell<Vec<Box<str>>>,
}

impl IntStrings {
    pub fn new() -> Self {
        Self {
            bufs: RefCell::new(vec![]),
        }
    }

    /// Returns a `&str` pointing into owned storage
    pub fn format(&self, num: u64) -> &str {
        let s: Box<str> = num.to_string().into_boxed_str();

        // Leak the string to get a reference that lives "forever"
        let s_ref: &str = Box::leak(s);

        // Store the box in the Vec so we can drop later
        self.bufs
            .borrow_mut()
            .push(unsafe { Box::from_raw(s_ref as *const str as *mut str) });

        s_ref
    }
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
