{ pkgs, ... }:

{

  cachix.enable = false;

  services.postgres = {
    enable = true;
    package = pkgs.postgresql_18;
    listen_addresses = "127.0.0.1";
    port = 5432;
    initialDatabases = [
      {
        name = "bk";
        pass = "password";
        user = "bk";
      }
    ];
  };

  enterShell = ''
    export DATABASE_URL="postgres://bk:password@127.0.0.1:5432/bk"
  '';

}
