use argh::FromArgs;

use crate::{config::Config, restic, run_command};

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

#[derive(Default)]
pub struct ModeSelection {
    rsync: bool,
    restic: bool,
    restic_forget: bool,
}

impl ModeSelection {
    pub fn from(i: Vec<String>) -> Self {
        if i.is_empty() {
            return Self {
                rsync: true,
                restic: true,
                restic_forget: true,
            };
        }

        let mut s = Self::default();

        log::info!("Running with modes {i:?}");

        for e in i {
            match e.to_lowercase().as_str() {
                "rsync" => s.rsync = true,
                "restic" => s.restic = true,
                "restic_forget" => s.restic_forget = true,
                _ => {
                    eprintln!("Unknown mode {e}");
                    std::process::exit(1);
                }
            }
        }

        s
    }
}

impl RunCommand {
    pub fn run(&self) {
        let conf = Config::from_path(&self.config);
        let mut state = 0;

        if self.dry_run {
            log::warn!("Running in dry run mode. No backup jobs will happen.");
        }

        if let Some(delay) = conf.delay {
            let wait = rand::random_range(0..delay);
            log::info!("Delaying backup for {wait} seconds...");
            std::thread::sleep(std::time::Duration::from_secs(wait));
        }

        if let Some(script) = &conf.start_script {
            run_command(&["sh", script.as_str()], None);
        }

        // mode selection
        let modes = ModeSelection::from(self.mode.clone());

        if modes.rsync {
            for rsync in &conf.rsync.unwrap_or_default() {
                rsync.run_backup(self.dry_run);
            }
        }

        // Restic backups
        if modes.restic {
            for restic in &conf.restic.unwrap_or_default() {
                if self.exclude.iter().any(|x| restic.options.src.contains(x)) {
                    log::info!(
                        "Skipping restic operation due to exclude filter: exclude {:?}, got {:?}",
                        self.exclude,
                        restic.options.src
                    );
                    continue;
                }

                let res = restic::create_archive(
                    restic,
                    conf.path.clone().unwrap_or_default(),
                    conf.restic_target.clone().unwrap_or_default(),
                    self.dry_run,
                    conf.home.clone(),
                );

                for (target, res) in res {
                    let notify_provider = conf.ntfy.clone().unwrap_or_default();

                    if let Err(e) = res {
                        log::error!("Backup to target {target} failed: {e}");
                        state = 1;

                        for ntfy_key in restic.ntfy.clone().unwrap_or_default() {
                            let ntfy_opt = notify_provider.get(&ntfy_key).unwrap();
                            ntfy_opt.send_notification(&format!(
                                "🚨 Backup failed for {} to {}: {e}",
                                restic.options.src.join(", "),
                                target
                            ));
                        }
                    } else {
                        log::info!("Backup successfull for {target}");

                        for ntfy_key in restic.ntfy.clone().unwrap_or_default() {
                            let ntfy_opt = notify_provider.get(&ntfy_key).unwrap();
                            ntfy_opt.send_notification(&format!(
                                "✅ Backup successful for {:?} to {}",
                                restic.options.src, target
                            ));
                        }
                    }
                }
            }
        }

        // Restic forget
        if modes.restic_forget {
            for restic in &conf.restic_forget.unwrap_or_default() {
                let res = restic::forget_archive(
                    restic,
                    conf.restic_target.clone().unwrap_or_default(),
                    self.dry_run,
                );

                for (target, res) in res {
                    let notify_provider = conf.ntfy.clone().unwrap_or_default();

                    if let Err(e) = res {
                        log::error!("Forget for target {target} failed: {e}");
                        state = 1;

                        for ntfy_key in restic.ntfy.clone().unwrap_or_default() {
                            let ntfy_opt = notify_provider.get(&ntfy_key).unwrap();
                            ntfy_opt.send_notification(&format!(
                                "🚨 Forget failed for {} to {}: {e}",
                                restic.targets.join(", "),
                                target
                            ));
                        }
                    } else {
                        log::info!("Forget successfull for {target}");

                        for ntfy_key in restic.ntfy.clone().unwrap_or_default() {
                            let ntfy_opt = notify_provider.get(&ntfy_key).unwrap();
                            ntfy_opt.send_notification(&format!(
                                "✅ Forget successful for {:?} to {}",
                                restic.targets, target
                            ));
                        }
                    }
                }
            }
        }

        if let Some(script) = &conf.end_script {
            run_command(&["sh", script.as_str()], None);
        }

        std::process::exit(state);
    }
}
