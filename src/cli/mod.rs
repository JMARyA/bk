use argh::FromArgs;
use facet_pretty::FacetPretty;

mod run;
use run::*;

use crate::{config::Config, restic, server::ServeCommand};

#[derive(FromArgs, PartialEq, Debug)]
/// Bk
pub struct BkArgs {
    #[argh(subcommand)]
    pub cmd: BkCommand,
}

impl BkArgs {
    pub fn run(&self) {
        self.cmd.run();
    }
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum BkCommand {
    Show(ShowCommand),
    Init(InitCommand),
    Run(RunCommand),
    List(ListCommand),
    Restore(RestoreCommand),
    ConfigSchema(ConfigSchema),
    Serve(ServeCommand),
}

impl BkCommand {
    pub fn run(&self) {
        match self {
            BkCommand::Show(c) => c.run(),
            BkCommand::Init(c) => c.run(),
            BkCommand::Run(c) => c.run(),
            BkCommand::List(c) => c.run(),
            BkCommand::Restore(c) => c.run(),
            BkCommand::ConfigSchema(c) => c.run(),
            BkCommand::Serve(c) => c.run(),
        }
    }
}

#[derive(FromArgs, PartialEq, Debug)]
/// Show config json schema
#[argh(subcommand, name = "config_schema")]
pub struct ConfigSchema {}

impl ConfigSchema {
    pub fn run(&self) {
        let schema = schemars::schema_for!(crate::config::Config);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    }
}

#[derive(FromArgs, PartialEq, Debug)]
/// Show config
#[argh(subcommand, name = "show")]
pub struct ShowCommand {
    #[argh(positional)]
    /// config file
    pub config: String,
}

impl ShowCommand {
    pub fn run(&self) {
        let conf = crate::config::Config::from_path(&self.config);
        // TODO : better representation
        println!("{}", conf.pretty());
    }
}

#[derive(FromArgs, PartialEq, Debug)]
/// Init repositories
#[argh(subcommand, name = "init")]
pub struct InitCommand {
    #[argh(positional)]
    /// config file
    pub config: String,
}

impl InitCommand {
    pub fn run(&self) {
        let conf = Config::from_path(&self.config);
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
}

#[derive(FromArgs, PartialEq, Debug)]
/// List snapshots
#[argh(subcommand, name = "list")]
pub struct ListCommand {
    #[argh(positional)]
    /// config file
    pub config: String,
}

impl ListCommand {
    pub fn run(&self) {
        let conf = Config::from_path(&self.config);
        for (name, target) in conf.restic_target.unwrap_or_default() {
            let snaps = target.get_snapshots().unwrap();
            println!("{name}: {}", snaps.pretty());
        }
    }
}

#[derive(FromArgs, PartialEq, Debug)]
/// Restore a snapshot to a local path
#[argh(subcommand, name = "restore")]
pub struct RestoreCommand {
    #[argh(positional)]
    /// config file
    pub config: String,

    #[argh(positional)]
    /// restic_target key from config
    pub target: String,

    #[argh(positional)]
    /// snapshot ID to restore (short or full)
    pub snapshot_id: String,

    #[argh(positional)]
    /// destination path to restore into
    pub destination: String,
}

impl RestoreCommand {
    pub fn run(&self) {
        let conf = Config::from_path(&self.config);
        let targets = conf.restic_target.unwrap_or_default();
        let target = match targets.get(&self.target) {
            Some(t) => t,
            None => {
                eprintln!("Unknown restic target: {}", self.target);
                std::process::exit(1);
            }
        };
        match restic::restore_archive(target, &self.snapshot_id, &self.destination) {
            Ok(()) => log::info!("Restore complete."),
            Err(e) => {
                log::error!("Restore failed: {e:?}");
                std::process::exit(1);
            }
        }
    }
}
