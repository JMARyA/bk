{
  config,
  lib,
  inputs,
  pkgs,
  ...
}:

let
  cfg = config.services.bk;
in
{
  options.services.bk = {
    enable = lib.mkEnableOption "bk service";

    settings = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      description = "bk.toml settings";
    };

  };

  config = lib.mkIf cfg.enable {

    environment.etc."bk.toml".source = pkgs.writers.writeTOML "bk.toml" cfg.settings;

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
