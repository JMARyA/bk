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
in
{
  options.services.bk = {
    enable = lib.mkEnableOption "bk service";

    state = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = "state paths to backup";
    };

    repo = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "backup repository";
    };

    repoOptions = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      description = "repository options";
    };

    globalSettings = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      description = "Top level options";
    };

    settings = lib.mkOption {
      type = lib.types.listOf lib.types.attrs;
      default = [ ];
      description = "bk.toml settings blocks";
    };

  };

  config = lib.mkIf cfg.enable {

    assertions = [
      {
        assertion = (cfg.state == [ ]) || (cfg.repo != null);
        message = "Repository can't be null if state is specified.";
      }
    ];

    environment.etc."bk.toml".source = pkgs.writers.writeTOML "bk.toml" (
      cfg.globalSettings
      // (
        bklib.mergeBkConf (
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
      )
    );

    # Backup service
    systemd.services.bk-run = {
      description = "Backup";
      after = [ "network.target" ];
      path = with pkgs; [
        restic
        util-linux
        coreutils
      ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${inputs.bk.packages.${pkgs.system}.default}/bin/bk run /etc/bk.toml";
        User = "root";
        Environment = "HOME=/root";
        StandardOutput = "journal";
        StandardError = "journal";
      };
    };

    # Backup timer
    systemd.timers.bk-run = {
      description = "Scheduled backup";
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnCalendar = "daily";
        Persistent = true;
      };
    };

  };
}
