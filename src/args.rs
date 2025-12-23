use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
/// Bk
pub struct BkArgs {
    #[argh(subcommand)]
    pub cmd: BkCommand,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum BkCommand {
    Show(ShowCommand),
    Run(RunCommand),
    ConfigSchema(ConfigSchema),
}

#[derive(FromArgs, PartialEq, Debug)]
/// Show config json schema
#[argh(subcommand, name = "config_schema")]
pub struct ConfigSchema {}

#[derive(FromArgs, PartialEq, Debug)]
/// Show config
#[argh(subcommand, name = "show")]
pub struct ShowCommand {
    #[argh(positional)]
    /// config file
    pub config: String,
}

#[derive(FromArgs, PartialEq, Debug, Default)]
/// Run config
#[argh(subcommand, name = "run")]
pub struct RunCommand {
    #[argh(positional)]
    /// config file
    pub config: String,

    #[argh(switch, short = 'n')]
    /// dry run
    pub dry_run: bool,

    #[argh(option, short = 'e')]
    /// exclude paths from operations
    pub exclude: Vec<String>,

    #[argh(option, short = 'm')]
    /// enable a mode of operation (module)
    pub mode: Vec<String>,
}
