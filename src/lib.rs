use yansi::{Color, Paint};

pub mod cli;
pub mod config;
pub mod input;
pub mod notify;
pub mod restic;
pub mod rsync;
pub mod server;

pub fn run_command(cmd: &[&str], env: Option<Vec<(String, String)>>) -> (String, String, i32) {
    println!("--> {} ", cmd.join(" ").paint(Color::Blue));

    let mut cmd_setup = std::process::Command::new(cmd[0]);
    let mut cmd_setup = cmd_setup.args(cmd.iter().skip(1).collect::<Vec<_>>());

    cmd_setup = cmd_setup
        .stdout(std::process::Stdio::inherit())
        .stdin(std::process::Stdio::inherit());

    if let Some(pw) = env {
        for e in pw {
            cmd_setup = cmd_setup.env(e.0, e.1);
        }
    }

    let child = cmd_setup.spawn().unwrap();

    let status = child.wait_with_output().unwrap();
    if !status.status.success() {
        println!(
            "{} Command {} returned with non zero exit code.",
            "Error:".paint(Color::Red),
            cmd.join(" ")
        );
    }

    let output = String::from_utf8(status.stdout).unwrap();
    let stderr = String::from_utf8(status.stderr).unwrap();

    if !stderr.trim().is_empty() {
        eprintln!("{stderr}");
    }

    (output, stderr, status.status.code().unwrap())
}

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
