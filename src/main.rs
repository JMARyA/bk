use bk::{args::BkArgs, backup::run_backup, config::Config};
use facet_pretty::FacetPretty;
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
            println!("{}", conf.pretty());
        }
        bk::args::BkCommand::Serve(srv) => {}
        bk::args::BkCommand::List(lst) => {
            let conf = Config::from_path(&lst.config);
            for (name, target) in conf.restic_target.unwrap_or_default() {
                let snaps = target.get_snapshots().unwrap();
                println!("{name}: {}", snaps.pretty());
            }
        }
        bk::args::BkCommand::Init(init) => {
            let conf = Config::from_path(&init.config);
            if let Some(targets) = conf.restic_target {
                for (_, target) in targets {
                    match target.init_repo() {
                        Ok(_) => {
                            println!("Initialized new restic repository!");
                        }
                        Err(_) => {
                            println!(
                                "Initializing new repository failed! Maybe the repository already exists?"
                            );
                        }
                    }
                }
            }
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
