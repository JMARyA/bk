use bk::{
    args::{BkArgs, RunCommand},
    backup::run_backup,
    config::Config,
};
use schemars::schema_for;

// TODO : add basic ctrl+c support for ending bk tasks instead of everything and ensure cleanups

fn main() {
    // Enable log output by default
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }
    env_logger::init();

    let args: BkArgs = argh::from_env();
    match args.cmd {
        bk::args::BkCommand::Show(show_command) => {
            let conf = Config::from_path(&show_command.config);
            // TODO : better representation
            println!("{conf:#?}");
        }
        bk::args::BkCommand::Run(run_command) => {
            let state = run_backup(run_command);
            std::process::exit(state);
        }
        bk::args::BkCommand::ConfigSchema(_) => {
            let schema = schema_for!(bk::config::Config);
            println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        }
    }
}
