use backup::run_backup;
use yansi::{Color, Paint};

mod backup;
mod borg;
mod config;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    if let Some(conf) = args.get(1) {
        if conf.as_str() == "init" {
            let repo = args.get(2).unwrap();
            borg::init_repo(repo);
            std::process::exit(0);
        }

        let conf = toml::from_str(&std::fs::read_to_string(conf).unwrap()).unwrap();
        run_backup(conf);
    } else {
        println!("Usage: bk <config>");
    }
}

pub fn run_command(cmd: &[&str], borg_passphrase: Option<String>) -> (String, String) {
    println!("--> {} ", cmd.join(" ").paint(Color::Blue));

    let mut cmd_setup = std::process::Command::new(cmd[0]);
    let mut cmd_setup = cmd_setup.args(cmd.iter().skip(1).collect::<Vec<_>>());

    cmd_setup = cmd_setup
        .stdout(std::process::Stdio::inherit())
        .stdin(std::process::Stdio::inherit());

    if let Some(pw) = borg_passphrase {
        cmd_setup = cmd_setup.env("BORG_PASSPHRASE", pw);
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

    (output, stderr)
}
