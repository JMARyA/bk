use std::{cell::RefCell, collections::HashMap};

mod binary;
mod error;
mod snapshot;
use binary::*;
use error::*;
use facet_pretty::FacetPretty;
pub use snapshot::*;

use cmdbind::{
    CommandEnvironment, RunnableCommand, errors::FromExitCode, validators::non_zero_only,
    wrap_binary,
};

use facet::Facet;
use miette::{IntoDiagnostic, Result};
use yansi::{Color, Paint};

use crate::{
    config::{LocalPath, LocalPathRef, ResticConfig, ResticForget, ResticForgetArgs, ResticTarget},
    run_command,
};

pub fn bind_mount(src: &str, dst: &str) {
    run_command(&["mount", "--bind", src, dst], None);
}

pub fn umount(mount: &str) {
    run_command(&["umount", mount], None);
}

impl ResticTarget {
    /// Initialize a new restic repository.
    ///
    /// # Returns
    ///
    /// * `Result<(), ResticError>` - A result indicating success or failure of the repository initialization process, wrapped in a `ResticError`.
    pub fn init_repo(&self) -> Result<(), ResticError> {
        log::info!(
            "Initializing restic repository on {}",
            self.repo.paint(Color::Yellow)
        );

        let mut cmd_args = ResticInitArgs {
            option: Vec::new(),
            json: false,
            repo: self.repo.clone(),
        };

        let env = match self.setup_env() {
            Err(e) => return Err(e),
            Ok((env, ssh_opt)) => {
                if let Some(ssh_opt) = ssh_opt {
                    cmd_args.option.push(ssh_opt);
                }
                env
            }
        };

        let cmd = ResticInitCommand::new(cmd_args);
        let res = cmd.run(Some(&env));

        match res {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => match e {
                cmdbind::errors::CommandError::Internal(_) => {
                    return Err(ResticError::Fatal);
                }
                cmdbind::errors::CommandError::Output(command_output) => {
                    let err = ResticError::from_code(command_output.status().unwrap()).unwrap();
                    return Err(err);
                }
            },
        }
    }

    pub fn setup_env(&self) -> Result<(CommandEnvironment, Option<String>), ResticError> {
        let mut env = CommandEnvironment::new();

        if let Some(passphrase) = &self.passphrase {
            env.env("RESTIC_PASSWORD".to_string(), passphrase.clone());
        } else if let Some(pass_file) = &self.passphrase_file {
            let passphrase =
                std::fs::read_to_string(pass_file).expect("Could not read passphrase file");
            env.env("RESTIC_PASSWORD".to_string(), passphrase);
        } else {
            log::error!(
                "Neither passphrase nor passphrase file provided for {}",
                self.repo
            );
            return Err(ResticError::Fatal);
        }

        if let Some(s3) = &self.s3 {
            env.env(
                "AWS_ACCESS_KEY_ID".to_string(),
                s3.access_key().expect(NO_S3_CREDS).clone(),
            );
            env.env(
                "AWS_SECRET_ACCESS_KEY".to_string(),
                s3.secret_key().expect(NO_S3_CREDS).clone(),
            );
        }

        let mut ssh_opt = None;

        if let Some(ssh) = &self.ssh {
            let remote = self.repo.trim_start_matches("sftp:");
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

        Ok((env, ssh_opt))
    }

    pub fn get_snapshots(&self) -> Result<Vec<Snapshot>, ResticError> {
        let (env, _) = self.setup_env()?;
        let x = ResticSnapshotsCommand::new(ResticSnapshotsArgs {
            positional_snapshot_id: Vec::new(),
            compact: false,
            group_by: None,
            host: None,
            latest: None,
            path: None,
            tag: None,
            json: true,
            no_lock: false,
            repo: self.repo.clone(),
        })
        .run(Some(&env))
        .unwrap();

        let x = x.stdout_str().unwrap();
        Ok(facet_json::from_str(&x).unwrap())
    }

    pub fn modify_tag(
        &self,
        snapshot_id: String,
        tag: String,
        remove: bool,
    ) -> Result<(String, String), ()> {
        if remove {
            log::info!(
                "Removing tag '{tag}' from snapshot '{snapshot_id}' on repository '{}'",
                self.repo
            );
        } else {
            log::info!(
                "Tagging snapshot '{snapshot_id}' on repository '{}' with tag '{tag}'",
                self.repo
            );
        }

        let mut args = if remove {
            ResticTagArgsBuilder::default()
                .remove(vec![tag])
                .build()
                .unwrap()
        } else {
            ResticTagArgsBuilder::default()
                .add(vec![tag])
                .build()
                .unwrap()
        };

        let env = match self.setup_env() {
            Err(e) => {
                return Err(());
            }
            Ok((env, ssh_opt)) => {
                if let Some(ssh_opt) = ssh_opt {
                    args.option.push(ssh_opt);
                }
                env
            }
        };

        args.repo = self.repo.clone();
        args.json = true;
        args.quiet = true;

        let ret = ResticTagCommand::new(args).run(Some(&env));

        if let Ok(o) = ret {
            let stdout = o.stdout_str().unwrap();
            let parsed = parse_json_lines(&stdout);
            for msg in parsed {
                match msg {
                    ResticMsgType::Status(restic_status_msg) => {}
                    ResticMsgType::Changed(restic_changed_msg) => {
                        let x = (
                            restic_changed_msg.old_snapshot_id,
                            restic_changed_msg.new_snapshot_id,
                        );
                        log::info!("Tagged '{}' -> '{}'", x.0, x.1);
                        return Ok(x);
                    }
                    ResticMsgType::Summary(restic_summary_msg) => {}
                }
            }
        }

        Err(())
    }
}

const NO_S3_CREDS: &str = "no s3 credentials provided";

#[derive(Debug, Default, serde::Serialize)]
pub struct HostnameArgs {}

wrap_binary!(HostnameCmd, "hostname", HostnameArgs, non_zero_only);

pub fn hostname() -> String {
    HostnameCmd::new(HostnameArgs {})
        .run(None)
        .unwrap()
        .stdout_str()
        .unwrap()
        .trim()
        .to_string()
}

pub fn parse_json_lines(input: &str) -> Vec<ResticMsgType> {
    let mut ret = Vec::new();

    for line in input.lines() {
        let x: ResticMsgType = facet_json::from_str(line).into_diagnostic().unwrap();
        ret.push(x);
    }

    ret
}

/// get the id of the machine.
/// This is the sha256 fingerprint of the ssh host key
pub fn machine_id() -> String {
    let key_data = std::fs::read_to_string("/etc/ssh/ssh_host_ed25519_key.pub").unwrap();
    let public_key = ssh_key::PublicKey::from_openssh(&key_data).unwrap();

    let fingerprint = public_key.fingerprint(Default::default());
    fingerprint
        .to_string()
        .trim_start_matches(&format!("{}:", fingerprint.prefix()))
        .to_string()
}

pub struct HeadTag {
    hostname: String,
    host_key: String,
}

impl HeadTag {
    /// get the self head marker for the current machine
    pub fn own() -> Self {
        Self {
            hostname: hostname(),
            host_key: machine_id(),
        }
    }

    pub fn to_string(&self) -> String {
        format!("head:{}:{}", self.hostname, self.host_key)
    }
}

pub fn is_head_tag(head: &HeadTag) -> impl Fn(Snapshot) -> Option<Snapshot> {
    move |x| {
        if x.tags.iter().any(|tag| tag.starts_with(&head.to_string())) {
            Some(x)
        } else {
            None
        }
    }
}

pub fn is_head(x: Snapshot) -> Option<Snapshot> {
    if x.tags.iter().any(|x| x.starts_with("head:")) {
        return Some(x);
    }

    None
}

pub fn create_archive(
    conf: &ResticConfig,
    path_provider: HashMap<String, LocalPath>,
    target_provider: HashMap<String, ResticTarget>,
    dry: bool,
) -> HashMap<String, Result<(), ResticError>> {
    // TODO : sanity checks on head tag / init logic

    // TODO : get previous head / init logic

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

        let mut cmd_args = ResticBackupCommandArgs::from_config(conf.clone());

        cmd_args.dry_run = dry;
        cmd_args.json = true;
        cmd_args.repo = repo.repo.clone();
        cmd_args
            .positional_0_dir
            .extend(dirs.iter().map(|x| x.to_string()));

        let env = match repo.setup_env() {
            Err(e) => {
                targets_results.insert(repo.repo.clone(), Err(e));
                continue;
            }
            Ok((env, ssh_opt)) => {
                if let Some(ssh_opt) = ssh_opt {
                    cmd_args.option.push(ssh_opt);
                }
                env
            }
        };

        let snapshots = repo.get_snapshots().unwrap();
        let snapshots: Vec<_> = snapshots
            .into_iter()
            .filter_map(is_head_tag(&HeadTag::own()))
            .collect();
        println!("{} : {}", snapshots.len(), snapshots.pretty());

        // TODO : resolve failure case : multiple heads
        if snapshots.len() > 1 {
            panic!("Multiple Heads 🐉");
        }

        let (_, parent) = if snapshots.len() == 0 {
            (String::new(), String::new())
        } else {
            repo.modify_tag(
                snapshots.first().unwrap().id.clone(),
                HeadTag::own().to_string(),
                true,
            )
            .unwrap()
        };

        log::info!("Found parent {parent}");

        // TODO : test tagging while creation, avoiding double rename, etc

        cmd_args.tag.push(HeadTag::own().to_string());
        cmd_args.tag.push(format!("parent:{parent}"));

        let cmd = ResticBackupCommand::new(cmd_args);
        let res = cmd.run(Some(&env));

        match res {
            Ok(out) => {
                let stdout = out.stdout_str().unwrap();
                for line in stdout.lines() {
                    let x: ResticMsgType = facet_json::from_str(line).into_diagnostic().unwrap();
                    if let ResticMsgType::Summary(summary) = x {}
                }
                targets_results.insert(repo.repo.clone(), Ok(()));
            }
            Err(e) => match e {
                cmdbind::errors::CommandError::Internal(_) => {
                    targets_results.insert(repo.repo.clone(), Err(ResticError::Fatal));
                }
                cmdbind::errors::CommandError::Output(command_output) => {
                    let err = ResticError::from_code(command_output.status().unwrap()).unwrap();
                    targets_results.insert(repo.repo.clone(), Err(err));
                }
            },
        }
    }

    targets_results
}

#[derive(Facet, Debug)]
pub struct ResticStatusMsg {
    pub percent_done: f64,
    pub total_files: i64,
    pub files_done: Option<i64>,
    pub total_bytes: i64,
    pub bytes_done: Option<i64>,
    pub seconds_remaining: Option<i64>,
}

#[derive(Facet, Debug)]
#[repr(u8)]
#[facet(untagged)]
pub enum ResticMsgType {
    Status(ResticStatusMsg),
    Changed(ResticChangedMsg),
    Summary(ResticSummaryMsg),
}

#[derive(Facet, Debug)]
pub struct ResticChangedMsg {
    pub old_snapshot_id: String,
    pub new_snapshot_id: String,
}

#[derive(Facet, Debug)]
pub struct ResticSummaryMsg {
    // Snapshot Edit Summary
    pub changed_snapshots: Option<i64>,

    // Backup Summary
    pub files_new: Option<i64>,
    pub files_changed: Option<i64>,
    pub files_unmodified: Option<i64>,
    pub dirs_new: Option<i64>,
    pub dirs_changed: Option<i64>,
    pub dirs_unmodified: Option<i64>,
    pub data_blobs: Option<i64>,
    pub tree_blobs: Option<i64>,
    pub data_added: Option<i64>,
    pub data_added_packed: Option<i64>,
    pub total_files_processed: Option<i64>,
    pub total_bytes_processed: Option<i64>,
    pub total_duration: Option<f64>,
    pub backup_start: Option<chrono::DateTime<chrono::Utc>>,
    pub backup_end: Option<chrono::DateTime<chrono::Utc>>,
    pub snapshot_id: Option<String>,
}

wrap_binary!(
    ResticForgetCommand,
    "restic",
    ResticForgetArgs,
    restic_backup_validator,
    "forget"
);

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

        let mut cmd_args = conf.args.clone();

        cmd_args.dry_run = dry;
        cmd_args.repo = Some(repo.repo.clone());

        let env = match repo.setup_env() {
            Err(e) => {
                targets_results.insert(repo.repo.clone(), Err(e));
                continue;
            }
            Ok((env, ssh_opt)) => {
                if let Some(ssh_opt) = ssh_opt {
                    cmd_args.option.push(ssh_opt);
                }
                env
            }
        };

        let res = ResticForgetCommand::new(cmd_args).run(Some(&env));

        match res {
            Ok(_) => {
                targets_results.insert(repo.repo.clone(), Ok(()));
            }
            Err(e) => match e {
                cmdbind::errors::CommandError::Internal(_) => {
                    targets_results.insert(repo.repo.clone(), Err(ResticError::Fatal));
                }
                cmdbind::errors::CommandError::Output(command_output) => {
                    let err = ResticError::from_code(command_output.status().unwrap()).unwrap();
                    targets_results.insert(repo.repo.clone(), Err(err));
                }
            },
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
