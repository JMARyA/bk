use bk::{
    args::{BkArgs, RunCommand},
    backup::run_backup,
    config::Config,
};

// TODO : add basic ctrl+c support for ending bk tasks instead of everything and ensure cleanups

fn main() {
    // Enable log output by default
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }
    env_logger::init();

    let args = std::env::args().collect::<Vec<_>>();

    if args.len() > 2 {
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
        }
    } else {
        let conf = args.get(1).unwrap();

        if conf == "-h" || conf.to_lowercase() == "--help" {
            let _: BkArgs = argh::from_env();
        }

        let state = run_backup(RunCommand {
            config: conf.to_string(),
            ..Default::default()
        });
        std::process::exit(state);
    }
}
