use std::{cell::RefCell, collections::HashMap};

mod binary;
mod error;
mod snapshot;
use binary::*;
use error::*;
pub use snapshot::*;

use cmdbind::{
    errors::FromExitCode, validators::non_zero_only, wrap_binary, CommandEnvironment,
    RunnableCommand,
};

use facet::Facet;
use miette::Result;
use yansi::{Color, Paint};

use crate::{
    config::{ResticBackupConfig, ResticConfig, ResticForget, ResticForgetArgs, ResticTarget},
    input::{InputRef, LocalPath, LocalPathRef, S3Input, S3InputRef},
    notify::post_api,
    run_command, server,
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
            let hostpart = match hostpart.first() {
                Some(h) => *h,
                None => {
                    log::error!("malformed SFTP repo URL: {}", self.repo);
                    return Err(ResticError::Fatal);
                }
            };
            let (user, host) = match hostpart.split_once('@') {
                Some(p) => p,
                None => {
                    log::error!(
                        "malformed SFTP repo URL (expected user@host): {}",
                        self.repo
                    );
                    return Err(ResticError::Fatal);
                }
            };
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

    /// Returns the env vars needed to run restic as a plain Vec, suitable for
    /// passing to `std::process::Command::envs`.
    pub fn env_vars(&self) -> Result<Vec<(String, String)>, ResticError> {
        let mut envs: Vec<(String, String)> = Vec::new();
        if let Some(pass) = &self.passphrase {
            envs.push(("RESTIC_PASSWORD".to_string(), pass.clone()));
        } else if let Some(pass_file) = &self.passphrase_file {
            let pass = std::fs::read_to_string(pass_file).map_err(|_| ResticError::Fatal)?;
            envs.push(("RESTIC_PASSWORD".to_string(), pass.trim().to_string()));
        } else {
            log::error!(
                "Neither passphrase nor passphrase file provided for {}",
                self.repo
            );
            return Err(ResticError::Fatal);
        }
        if let Some(s3) = &self.s3 {
            envs.push((
                "AWS_ACCESS_KEY_ID".to_string(),
                s3.access_key().expect(NO_S3_CREDS).clone(),
            ));
            envs.push((
                "AWS_SECRET_ACCESS_KEY".to_string(),
                s3.secret_key().expect(NO_S3_CREDS).clone(),
            ));
        }
        Ok(envs)
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
        facet_json::from_str(&x).map_err(|e| {
            log::error!("failed to parse restic snapshots output: {e}");
            ResticError::Fatal
        })
    }
}

const NO_S3_CREDS: &str = "no s3 credentials provided";

#[derive(Debug, Default, serde::Serialize)]
pub struct HostnameArgs {}

wrap_binary!(HostnameCmd, "hostname", HostnameArgs, non_zero_only);

pub fn hostname() -> String {
    // Use libc::gethostname to avoid depending on the external `hostname` binary,
    // which may not be present in minimal containers.
    let mut buf = [0u8; 256];
    let ret = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if ret == 0 {
        let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        if len > 0 {
            return String::from_utf8_lossy(&buf[..len]).trim().to_string();
        }
    }
    // Fallback: try /etc/hostname
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| {
            log::warn!("could not determine hostname via libc or /etc/hostname");
            "unknown".to_string()
        })
}

/// get the id of the machine.
/// This is the sha256 fingerprint of the ssh host key
pub fn machine_id() -> String {
    let key_path = "/etc/ssh/ssh_host_ed25519_key.pub";
    let key_data = match std::fs::read_to_string(key_path) {
        Ok(d) => d,
        Err(e) => {
            log::debug!("could not read ssh host key for machine_id: {e}");
            return String::new();
        }
    };
    let public_key = match ssh_key::PublicKey::from_openssh(&key_data) {
        Ok(k) => k,
        Err(e) => {
            log::warn!("could not parse ssh host key for machine_id: {e}");
            return String::new();
        }
    };
    let fingerprint = public_key.fingerprint(Default::default());
    fingerprint
        .to_string()
        .trim_start_matches(&format!("{}:", fingerprint.prefix()))
        .to_string()
}

pub fn create_archive(
    conf: &ResticConfig,
    path_provider: HashMap<String, LocalPath>,
    s3_input_provider: HashMap<String, S3Input>,
    target_provider: HashMap<String, ResticTarget>,
    dry: bool,
    home: Option<String>,
) -> HashMap<String, Result<(), ResticError>> {
    let mut paths: Vec<InputRef> = conf
        .options
        .src
        .iter()
        .map(|x| {
            if let Some(pp) = path_provider.get(x) {
                InputRef::Local(LocalPathRef::from(pp.clone()))
            } else if let Some(s3) = s3_input_provider.get(x) {
                InputRef::S3(S3InputRef::new(x, s3.clone()))
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
        targets_results.insert(
            repo.repo.clone(),
            create_archive_target(&conf.options, dirs.clone(), repo, dry, home.clone()),
        );
    }

    targets_results
}

pub fn create_archive_target(
    conf: &ResticBackupConfig,
    src: Vec<String>,
    target: &ResticTarget,
    dry: bool,
    home: Option<String>,
) -> Result<(), ResticError> {
    let mut cmd_args = ResticBackupCommandArgs::from_config(conf.clone());
    cmd_args.dry_run = dry;
    cmd_args.json = true;
    cmd_args.repo = target.repo.clone();
    cmd_args
        .positional_0_dir
        .extend(src.iter().map(|x| x.to_string()));

    // Get ssh_opt from setup_env (reuses existing auth/env validation path),
    // then get plain env vars for the streaming spawn.
    let ssh_opt = match target.setup_env() {
        Ok((_, ssh_opt)) => ssh_opt,
        Err(e) => return Err(e),
    };
    if let Some(opt) = ssh_opt {
        cmd_args.option.push(opt);
    }
    let envs = target.env_vars()?;

    match run_backup_streaming(&cmd_args, envs, &src, &target.repo) {
        Ok(summary) => {
            if let Some(home) = &home {
                emit_backup_summary(
                    home,
                    BackupEmitSummary::new(
                        target.repo.clone(),
                        src.clone(),
                        BackupState::Ok,
                        summary,
                    ),
                );
                emit_snapshots(home, target);
            }
            Ok(())
        }
        Err(e) => {
            if let Some(home) = &home {
                emit_backup_summary(
                    home,
                    BackupEmitSummary {
                        target: target.repo.clone(),
                        src: src.clone(),
                        status: BackupState::Error.to_string(),
                        summary: None,
                    },
                );
            }
            Err(e)
        }
    }
}

fn fmt_bytes(bytes: i64) -> String {
    let b = bytes.max(0) as f64;
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    if b >= GIB {
        format!("{:.1} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.1} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.0} KiB", b / KIB)
    } else {
        format!("{} B", bytes)
    }
}

fn fmt_num(n: i64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn fmt_eta(secs: i64) -> String {
    if secs >= 3600 {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

fn run_backup_streaming(
    cmd_args: &ResticBackupCommandArgs,
    envs: Vec<(String, String)>,
    src: &[String],
    repo: &str,
) -> Result<ResticSummaryMsg, ResticError> {
    use indicatif::{ProgressBar, ProgressStyle};
    use std::io::BufRead;

    // Build CLI args from the already-configured struct.
    let mut args: Vec<std::ffi::OsString> = vec!["backup".into()];
    args.push(format!("--repo={}", cmd_args.repo).into());
    args.push("--json".into());
    if cmd_args.dry_run {
        args.push("--dry-run".into());
    }
    if cmd_args.one_file_system {
        args.push("--one-file-system".into());
    }
    if cmd_args.exclude_caches {
        args.push("--exclude-caches".into());
    }
    if cmd_args.reread {
        args.push("--force".into());
    }
    if !cmd_args.compression.is_empty() {
        args.push(format!("--compression={}", cmd_args.compression).into());
    }
    if cmd_args.read_concurrency > 0 {
        args.push(format!("--read-concurrency={}", cmd_args.read_concurrency).into());
    }
    for tag in &cmd_args.tag {
        args.push(format!("--tag={tag}").into());
    }
    if let Some(excludes) = &cmd_args.exclude {
        for e in excludes {
            args.push(format!("--exclude={e}").into());
        }
    }
    if let Some(excludes) = &cmd_args.exclude_if_present {
        for e in excludes {
            args.push(format!("--exclude-if-present={e}").into());
        }
    }
    for opt in &cmd_args.option {
        args.push(format!("--option={opt}").into());
    }
    if let Some(host) = &cmd_args.host {
        args.push(format!("--host={host}").into());
    }
    for dir in &cmd_args.positional_0_dir {
        args.push(dir.into());
    }

    let mut child = std::process::Command::new("restic")
        .args(&args)
        .envs(envs)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| ResticError::Fatal)?;

    // Header line
    eprintln!(
        "\n  {} {}  →  {}",
        "▶ Backup".paint(Color::Cyan).bold(),
        src.join(", ").paint(Color::White).bold(),
        repo.paint(Color::Yellow),
    );

    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} [{bar:48.cyan/black}] {pos:>3}%  {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", ""])
            .progress_chars("█▉▊▋▌▍▎▏ "),
    );
    pb.set_position(0);
    pb.set_message("starting…".to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));

    let reader = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut summary = None;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        match facet_json::from_str::<ResticMsgType>(&line) {
            Ok(ResticMsgType::Status(s)) => {
                let pct = (s.percent_done * 100.0).clamp(0.0, 100.0) as u64;
                pb.set_position(pct);

                let files_part = match s.files_done {
                    Some(done) => format!("{}/{} files", fmt_num(done), fmt_num(s.total_files)),
                    None => format!("{} files", fmt_num(s.total_files)),
                };
                let bytes_part = match s.bytes_done {
                    Some(done) => format!("{}/{}", fmt_bytes(done), fmt_bytes(s.total_bytes)),
                    None => fmt_bytes(s.total_bytes),
                };
                let eta_part = match s.seconds_remaining {
                    Some(secs) if secs > 0 => {
                        format!("  eta {}", fmt_eta(secs).paint(Color::Yellow))
                    }
                    _ => String::new(),
                };
                pb.set_message(format!(
                    "{}  {}{}",
                    files_part.paint(Color::White),
                    bytes_part.paint(Color::Cyan),
                    eta_part,
                ));
            }
            Ok(ResticMsgType::Summary(s)) => {
                summary = Some(s);
            }
            Err(_) => {}
        }
    }

    let status = child.wait().map_err(|_| ResticError::Fatal)?;
    pb.finish_and_clear();

    if !status.success() {
        eprintln!(
            "  {} restic exited with code {}",
            "✗ Error:".paint(Color::Red).bold(),
            status.code().unwrap_or(-1),
        );
        let code = status.code().unwrap_or(1);
        return Err(ResticError::from_code(code).unwrap_or(ResticError::Fatal));
    }

    if let Some(ref s) = summary {
        let duration = s.total_duration.unwrap_or(0.0);
        let files = s.total_files_processed.unwrap_or(0);
        let bytes = s.total_bytes_processed.unwrap_or(0);
        let added = s.data_added_packed.unwrap_or(0);
        let snap = s
            .snapshot_id
            .as_deref()
            .map(|id| {
                format!(
                    "  snapshot {}",
                    &id[..8.min(id.len())].paint(Color::Magenta)
                )
            })
            .unwrap_or_default();
        eprintln!(
            "  {}  {:.1}s  ·  {} files  ·  {}  ·  {} added{}",
            "✔ Done".paint(Color::Green).bold(),
            duration,
            fmt_num(files).paint(Color::White),
            fmt_bytes(bytes).paint(Color::Cyan),
            fmt_bytes(added).paint(Color::Green),
            snap,
        );
    }
    eprintln!();

    summary.ok_or(ResticError::Fatal)
}

fn run_restore_streaming(
    snapshot_id: &str,
    destination: &str,
    repo: &str,
    envs: Vec<(String, String)>,
    opts: &[String],
) -> Result<(), ResticError> {
    use indicatif::{ProgressBar, ProgressStyle};
    use std::io::BufRead;

    let mut args: Vec<std::ffi::OsString> = vec!["restore".into()];
    args.push(format!("--repo={}", repo).into());
    args.push("--json".into());
    args.push(format!("--target={}", destination).into());
    for opt in opts {
        args.push(format!("--option={opt}").into());
    }
    args.push(snapshot_id.into());

    let mut child = std::process::Command::new("restic")
        .args(&args)
        .envs(envs)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| ResticError::Fatal)?;

    eprintln!(
        "\n  {} {}  →  {}",
        "▶ Restore".paint(Color::Cyan).bold(),
        snapshot_id.paint(Color::Magenta),
        destination.paint(Color::Yellow),
    );

    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} [{bar:48.cyan/black}] {pos:>3}%  {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", ""])
            .progress_chars("█▉▊▋▌▍▎▏ "),
    );
    pb.set_position(0);
    pb.set_message("starting…");
    pb.enable_steady_tick(std::time::Duration::from_millis(80));

    let reader = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut last_total_files = 0i64;
    let mut last_total_bytes = 0i64;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        match facet_json::from_str::<RestoreMsgType>(&line) {
            Ok(RestoreMsgType::Status(s)) => {
                let pct = (s.percent_done * 100.0).clamp(0.0, 100.0) as u64;
                pb.set_position(pct);
                last_total_files = s.total_files;
                last_total_bytes = s.total_bytes;
                let files_part = match s.files_restored {
                    Some(done) => format!("{}/{} files", fmt_num(done), fmt_num(s.total_files)),
                    None => format!("{} files", fmt_num(s.total_files)),
                };
                let bytes_part = match s.bytes_restored {
                    Some(done) => format!("{}/{}", fmt_bytes(done), fmt_bytes(s.total_bytes)),
                    None => fmt_bytes(s.total_bytes),
                };
                pb.set_message(format!(
                    "{}  {}",
                    files_part.paint(Color::White),
                    bytes_part.paint(Color::Cyan),
                ));
            }
            Ok(RestoreMsgType::Summary(s)) => {
                last_total_files = s.total_files;
                last_total_bytes = s.total_bytes;
            }
            Err(_) => {}
        }
    }

    let status = child.wait().map_err(|_| ResticError::Fatal)?;
    pb.finish_and_clear();

    if !status.success() {
        eprintln!(
            "  {} restic exited with code {}",
            "✗ Error:".paint(Color::Red).bold(),
            status.code().unwrap_or(-1),
        );
        let code = status.code().unwrap_or(1);
        return Err(ResticError::from_code(code).unwrap_or(ResticError::Fatal));
    }

    eprintln!(
        "  {}  {} files  ·  {}",
        "✔ Done".paint(Color::Green).bold(),
        fmt_num(last_total_files).paint(Color::White),
        fmt_bytes(last_total_bytes).paint(Color::Cyan),
    );
    eprintln!();

    Ok(())
}

pub fn restore_archive(
    target: &ResticTarget,
    snapshot_id: &str,
    destination: &str,
) -> Result<(), ResticError> {
    let mut opts = vec![];
    let ssh_opt = match target.setup_env() {
        Err(e) => return Err(e),
        Ok((_, ssh_opt)) => ssh_opt,
    };
    if let Some(o) = ssh_opt {
        opts.push(o);
    }
    let envs = target.env_vars()?;
    run_restore_streaming(snapshot_id, destination, &target.repo, envs, &opts)
}

fn signing_key_path() -> String {
    // SAFETY: getuid() is always safe to call.
    if unsafe { libc::getuid() } == 0 {
        "/etc/ssh/ssh_host_ed25519_key".to_string()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.ssh/id_ed25519")
    }
}

pub fn ssh_sign(data: &str) -> Option<String> {
    crate::ssh::ssh_sign(data.as_bytes(), &signing_key_path())
}

#[derive(Facet)]
pub struct BackupEmitSummary {
    pub target: String,
    pub src: Vec<String>,
    pub status: String,
    pub summary: Option<ResticSummaryMsg>,
}

pub enum BackupState {
    Ok,
    Error,
}

impl BackupState {
    pub fn to_string(&self) -> String {
        match self {
            Self::Ok => "OK".to_string(),
            Self::Error => "error".to_string(),
        }
    }
}

impl BackupEmitSummary {
    pub fn new(
        target: String,
        src: Vec<String>,
        status: BackupState,
        summary: ResticSummaryMsg,
    ) -> Self {
        Self {
            target,
            src,
            status: status.to_string(),
            summary: Some(summary),
        }
    }
}

fn make_signed_msg(
    kind: server::MsgKind,
    payload: String,
    public_key: &str,
) -> Option<server::StateMessage> {
    let sig = ssh_sign(&payload)?;
    Some(server::StateMessage {
        kind,
        hostname: hostname(),
        fingerprint: machine_id(),
        public_key: public_key.to_string(),
        payload,
        signature: sig,
    })
}

pub fn emit_backup_summary(home: &str, summary: BackupEmitSummary) {
    let public_key = match std::fs::read_to_string(format!("{}.pub", signing_key_path())) {
        Ok(k) => k.trim().to_string(),
        Err(e) => {
            log::warn!("could not read public key for backup emit: {e}");
            return;
        }
    };
    let payload = facet_json::to_string(&summary).unwrap();
    if let Some(msg) = make_signed_msg(server::MsgKind::Backup, payload, &public_key) {
        let _ = post_api(
            &format!("{home}/emit"),
            &serde_json::to_string(&msg).unwrap(),
            None,
        );
    }
}

pub fn emit_forget_summary(
    home: &str,
    target: &str,
    removed: usize,
    kept: usize,
    dry_run: bool,
    status: &str,
) {
    let public_key = match std::fs::read_to_string(format!("{}.pub", signing_key_path())) {
        Ok(k) => k.trim().to_string(),
        Err(e) => {
            log::warn!("could not read public key for forget emit: {e}");
            return;
        }
    };
    let summary = server::ForgetEmitSummary {
        target: target.to_string(),
        removed,
        kept,
        dry_run,
        status: status.to_string(),
    };
    let payload = serde_json::to_string(&summary).unwrap();
    if let Some(msg) = make_signed_msg(server::MsgKind::Forget, payload, &public_key) {
        let _ = post_api(
            &format!("{home}/emit"),
            &serde_json::to_string(&msg).unwrap(),
            None,
        );
    }
}

pub fn emit_snapshots(home: &str, target: &ResticTarget) {
    let public_key = match std::fs::read_to_string(format!("{}.pub", signing_key_path())) {
        Ok(k) => k.trim().to_string(),
        Err(e) => {
            log::warn!("could not read public key for snapshot sync: {e}");
            return;
        }
    };

    let envs = match target.env_vars() {
        Ok(e) => e,
        Err(e) => {
            log::warn!("could not get env for snapshot sync: {e:?}");
            return;
        }
    };

    let mut args: Vec<std::ffi::OsString> = vec![
        "snapshots".into(),
        format!("--repo={}", target.repo).into(),
        "--json".into(),
    ];

    // pull ssh_opt if needed
    if let Ok((_, Some(opt))) = target.setup_env() {
        args.push(format!("--option={opt}").into());
    }

    let out = std::process::Command::new("restic")
        .args(&args)
        .envs(envs)
        .output();

    let stdout = match out {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => {
            log::warn!(
                "restic snapshots exited {}: {}",
                o.status,
                String::from_utf8_lossy(&o.stderr)
            );
            return;
        }
        Err(e) => {
            log::warn!("failed to run restic snapshots: {e}");
            return;
        }
    };

    // Parse just what the server needs (id, short_id, hostname, paths, tags, time, username)
    let raw: serde_json::Value = match serde_json::from_slice(&stdout) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("failed to parse snapshots JSON: {e}");
            return;
        }
    };

    // Re-serialize as the slim SnapshotEntry list the server expects
    let entries: Vec<server::SnapshotEntry> = raw
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|s| {
            Some(server::SnapshotEntry {
                id: s.get("id")?.as_str()?.to_string(),
                short_id: s.get("short_id")?.as_str()?.to_string(),
                hostname: s
                    .get("hostname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                paths: s
                    .get("paths")?
                    .as_array()?
                    .iter()
                    .filter_map(|p| p.as_str().map(str::to_string))
                    .collect(),
                tags: s
                    .get("tags")
                    .and_then(|t| t.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| t.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                time: s.get("time")?.as_str()?.to_string(),
                username: s
                    .get("username")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            })
        })
        .collect();

    let payload = serde_json::to_string(&entries).unwrap();
    if let Some(msg) = make_signed_msg(server::MsgKind::Snapshots, payload, &public_key) {
        let _ = post_api(
            &format!("{home}/emit"),
            &serde_json::to_string(&msg).unwrap(),
            None,
        );
        log::info!("📸 Synced {} snapshots to server", entries.len());
    }
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
    Summary(ResticSummaryMsg),
}

#[derive(Facet, Debug)]
pub struct RestoreStatusMsg {
    pub percent_done: f64,
    pub total_files: i64,
    pub files_restored: Option<i64>,
    pub total_bytes: i64,
    pub bytes_restored: Option<i64>,
}

#[derive(Facet, Debug)]
pub struct RestoreSummaryMsg {
    pub total_files: i64,
    pub files_restored: i64,
    pub total_bytes: i64,
    pub bytes_restored: i64,
}

#[derive(Facet, Debug)]
#[repr(u8)]
#[facet(untagged)]
pub enum RestoreMsgType {
    Status(RestoreStatusMsg),
    Summary(RestoreSummaryMsg),
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

/// Returns `(removed, kept)` on success.
fn run_forget_streaming(
    args: &ResticForgetArgs,
    envs: Vec<(String, String)>,
    repo: &str,
) -> Result<(usize, usize), ResticError> {
    use indicatif::{ProgressBar, ProgressStyle};
    use std::io::BufRead;

    let mut cli: Vec<std::ffi::OsString> = vec!["forget".into()];
    cli.push(format!("--repo={}", repo).into());
    cli.push("--json".into());
    if args.dry_run {
        cli.push("--dry-run".into());
    }
    macro_rules! keep_flag {
        ($field:expr, $name:literal) => {
            if let Some(n) = $field {
                cli.push(format!("--{}={}", $name, n).into());
            }
        };
    }
    keep_flag!(args.keep_last, "keep-last");
    keep_flag!(args.keep_hourly, "keep-hourly");
    keep_flag!(args.keep_daily, "keep-daily");
    keep_flag!(args.keep_weekly, "keep-weekly");
    keep_flag!(args.keep_monthly, "keep-monthly");
    keep_flag!(args.keep_yearly, "keep-yearly");
    keep_flag!(args.keep_within, "keep-within");
    keep_flag!(args.keep_within_hourly, "keep-within-hourly");
    keep_flag!(args.keep_within_daily, "keep-within-daily");
    keep_flag!(args.keep_within_weekly, "keep-within-weekly");
    keep_flag!(args.keep_within_monthly, "keep-within-monthly");
    keep_flag!(args.keep_within_yearly, "keep-within-yearly");
    if let Some(true) = args.prune {
        cli.push("--prune".into());
    }
    if let Some(s) = &args.max_unused {
        cli.push(format!("--max-unused={s}").into());
    }
    if let Some(s) = &args.group_by {
        cli.push(format!("--group-by={s}").into());
    }
    if let Some(tags) = &args.keep_tag {
        for t in tags {
            cli.push(format!("--keep-tag={t}").into());
        }
    }
    if let Some(hosts) = &args.host {
        for h in hosts {
            cli.push(format!("--host={h}").into());
        }
    }
    if let Some(tags) = &args.tag {
        for t in tags {
            cli.push(format!("--tag={t}").into());
        }
    }
    if let Some(paths) = &args.path {
        for p in paths {
            cli.push(format!("--path={p}").into());
        }
    }
    for opt in &args.option.clone().unwrap_or_default() {
        cli.push(format!("--option={opt}").into());
    }

    eprintln!(
        "\n  {} {}",
        "▶ Forget".paint(Color::Cyan).bold(),
        repo.paint(Color::Yellow),
    );

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan}  {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", ""]),
    );
    pb.set_message("running…");
    pb.enable_steady_tick(std::time::Duration::from_millis(80));

    log::debug!("restic forget args: {:?}", cli);

    let mut child = std::process::Command::new("restic")
        .args(&cli)
        .envs(envs)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| ResticError::Fatal)?;

    let reader = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut stderr_reader = std::io::BufReader::new(child.stderr.take().unwrap());
    let mut lines: Vec<String> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        // Show prune progress inline when --prune is set
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            if v.get("message_type").and_then(|t| t.as_str()) == Some("status") {
                let pct = v
                    .get("percent_done")
                    .and_then(|p| p.as_f64())
                    .unwrap_or(0.0);
                pb.set_message(format!("pruning… {:.0}%", pct * 100.0));
            }
        }
        lines.push(line);
    }

    let status = child.wait().map_err(|_| ResticError::Fatal)?;
    pb.finish_and_clear();

    if !status.success() {
        let mut stderr_out = String::new();
        let _ = std::io::Read::read_to_string(&mut stderr_reader, &mut stderr_out);

        // Extract human-readable message from restic's JSON error lines if present,
        // otherwise fall back to the raw line.
        let error_msg: String = stderr_out
            .trim()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
                    .unwrap_or_else(|| line.to_owned())
            })
            .collect::<Vec<_>>()
            .join("\n");

        eprintln!(
            "  {} {}",
            "✗".paint(Color::Red).bold(),
            error_msg.paint(Color::Red),
        );
        log::debug!("forget stdout: {:?}", lines);
        let code = status.code().unwrap_or(1);
        return Err(ResticError::from_code(code).unwrap_or(ResticError::Fatal));
    }

    // Parse the forget JSON array (first line starting with '[')
    let (removed, kept) = lines
        .iter()
        .find(|l| l.trim_start().starts_with('['))
        .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .and_then(|v| v.as_array().cloned())
        .map(|groups| {
            let r: usize = groups
                .iter()
                .map(|g| {
                    g.get("remove")
                        .and_then(|x| x.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0)
                })
                .sum();
            let k: usize = groups
                .iter()
                .map(|g| {
                    g.get("keep")
                        .and_then(|x| x.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0)
                })
                .sum();
            (r, k)
        })
        .unwrap_or((0, 0));

    if args.dry_run {
        eprintln!(
            "  {}  {} to remove  ·  {} to keep  {}",
            "✔ Dry run".paint(Color::Yellow).bold(),
            removed.to_string().paint(Color::Red),
            kept.to_string().paint(Color::Green),
            "(no changes made)".paint(Color::Yellow),
        );
    } else {
        eprintln!(
            "  {}  {} removed  ·  {} kept",
            "✔ Done".paint(Color::Green).bold(),
            removed.to_string().paint(Color::Red),
            kept.to_string().paint(Color::Green),
        );
    }
    eprintln!();

    Ok((removed, kept))
}

pub fn forget_archive(
    conf: &ResticForget,
    target_provider: HashMap<String, ResticTarget>,
    dry: bool,
    home: Option<String>,
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
        let mut cmd_args = conf.args.clone();
        cmd_args.dry_run = dry;

        let ssh_opt = match repo.setup_env() {
            Err(e) => {
                targets_results.insert(repo.repo.clone(), Err(e));
                continue;
            }
            Ok((_, ssh_opt)) => ssh_opt,
        };
        if let Some(opt) = ssh_opt {
            cmd_args.option.get_or_insert_with(Vec::new).push(opt);
        }
        let envs = match repo.env_vars() {
            Err(e) => {
                targets_results.insert(repo.repo.clone(), Err(e));
                continue;
            }
            Ok(e) => e,
        };

        let res = run_forget_streaming(&cmd_args, envs, &repo.repo);
        match &res {
            Ok((removed, kept)) => {
                if let Some(h) = &home {
                    emit_forget_summary(h, &repo.repo, *removed, *kept, dry, "ok");
                }
            }
            Err(_) => {
                if let Some(h) = &home {
                    emit_forget_summary(h, &repo.repo, 0, 0, dry, "error");
                }
            }
        }
        targets_results.insert(repo.repo.clone(), res.map(|_| ()));
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
