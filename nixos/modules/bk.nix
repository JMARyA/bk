{
  config,
  lib,
  inputs,
  pkgs,
  ...
}:

let
  cfg = config.services.bk;
  bklib = import ../lib.nix;

  # Full merged config (all jobs combined)
  fullConfig =
    cfg.globalSettings
    // (bklib.mergeBkConf (
      (
        if cfg.state != [ ] then
          [
            (bklib.makeBk {
              paths = cfg.state;
              repo = cfg.repo;
              extraTargetOptions = cfg.repoOptions;
            })
          ]
        else
          [ ]
      )
      ++ cfg.settings
    ));

  # Shared between backup and prune: targets, paths, ntfy, and any scalar globals.
  # Excludes the job lists themselves so each unit only sees its own work.
  commonConfig = lib.filterAttrs (k: _: !builtins.elem k [ "restic" "restic_forget" ]) fullConfig;

  commonConfigFile = pkgs.writers.writeTOML "bk-common.toml" commonConfig;

  backupConfigFile = pkgs.writers.writeTOML "bk-backup.toml" (
    { imports = [ "${commonConfigFile}" ]; }
    // lib.optionalAttrs (fullConfig ? restic) { inherit (fullConfig) restic; }
  );

  pruneConfigFile = pkgs.writers.writeTOML "bk-prune.toml" (
    { imports = [ "${commonConfigFile}" ]; }
    // lib.optionalAttrs (fullConfig ? restic_forget) { inherit (fullConfig) restic_forget; }
  );

  bkPackage = inputs.bk.packages.${pkgs.system}.default;

  commonPath = with pkgs; [
    restic
    geesefs
    openssh
    rsync
    util-linux
    coreutils
  ];

  commonServiceConfig = {
    Type = "oneshot";
    User = "root";
    Environment = [
      "HOME=/root"
      "PATH=${lib.makeBinPath commonPath}"
    ];
    StandardOutput = "journal";
    StandardError = "journal";
  };
in
{
  options.services.bk = {
    enable = lib.mkEnableOption "bk backup service";

    state = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = "State paths to back up.";
    };

    repo = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Restic repository path or URL.";
    };

    repoOptions = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      description = "Extra restic_target options (e.g. passphrase).";
    };

    globalSettings = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      description = "Top-level config fields (e.g. home, delay).";
    };

    settings = lib.mkOption {
      type = lib.types.listOf lib.types.attrs;
      default = [ ];
      description = "Additional bk config blocks merged into the full config.";
    };

    backupTimer = lib.mkOption {
      type = lib.types.str;
      default = "daily";
      description = "OnCalendar expression for the backup timer (systemd syntax).";
      example = "*-*-* 02:00:00";
    };

    pruneTimer = lib.mkOption {
      type = lib.types.str;
      default = "weekly";
      description = "OnCalendar expression for the prune timer (systemd syntax).";
      example = "Sun *-*-* 03:00:00";
    };

    # Read-only rendered config for inspection / debugging
    config = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      readOnly = true;
      description = "Rendered merged config (read-only, for reference).";
    };
  };

  config = lib.mkIf cfg.enable {

    assertions = [
      {
        assertion = (cfg.state == [ ]) || (cfg.repo != null);
        message = "services.bk.repo must be set when services.bk.state is non-empty.";
      }
    ];

    services.bk.config = fullConfig;

    environment.etc = {
      "bk/common.toml".source = commonConfigFile;
      "bk/backup.toml".source = backupConfigFile;
      "bk/prune.toml".source = pruneConfigFile;
    };

    # ── Backup ────────────────────────────────────────────────────────────────

    systemd.services.bk-backup = {
      description = "bk restic backup";
      after = [ "network.target" ];
      serviceConfig = commonServiceConfig // {
        ExecStart = "${bkPackage}/bin/bk run -m restic /etc/bk/backup.toml";
      };
    };

    systemd.timers.bk-backup = {
      description = "bk backup timer";
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnCalendar = cfg.backupTimer;
        Persistent = true;
      };
    };

    # ── Prune ─────────────────────────────────────────────────────────────────

    systemd.services.bk-prune = {
      description = "bk restic prune";
      after = [ "network.target" ];
      serviceConfig = commonServiceConfig // {
        ExecStart = "${bkPackage}/bin/bk run -m restic_forget /etc/bk/prune.toml";
      };
    };

    systemd.timers.bk-prune = {
      description = "bk prune timer";
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnCalendar = cfg.pruneTimer;
        Persistent = true;
      };
    };

  };
}
