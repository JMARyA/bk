use yansi::{Color, Paint};

pub mod args;
pub mod backup;
pub mod config;
pub mod notify;
pub mod restic;
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
