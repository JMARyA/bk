{
  description = "Bk Backup Utility";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      ...
    }@inputs:
    {

      nixosModules.bk = import ./nixos/modules/bk.nix;

      lib = import ./nixos/lib.nix;

    }
    // flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib;
        craneLib = crane.mkLib pkgs;

        unfilteredRoot = ./.;
        src = lib.fileset.toSource {
          root = unfilteredRoot;
          fileset = lib.fileset.unions [
            (craneLib.fileset.commonCargoSources unfilteredRoot)
            ./migrations
          ];
        };

        commonArgs = {
          inherit src;
          strictDeps = true;

          OPENSSL_NO_VENDOR = "1";

          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";

          nativeBuildInputs = [
            pkgs.pkg-config
          ];

          buildInputs = [
            pkgs.openssl
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];
        };

        bk = craneLib.buildPackage (
          commonArgs
          // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          }
        );

        dockerImage = pkgs.dockerTools.buildLayeredImage {
          name = "bk";
          tag = "latest-${pkgs.stdenv.hostPlatform.linuxArch}";
          contents = [
            bk
            pkgs.restic
            pkgs.rsync
            pkgs.openssh
            pkgs.coreutils
            pkgs.util-linux
            pkgs.bash
          ];
          config = {
            Cmd = [ "/bin/bk" ];
            WorkingDir = "/app";
          };

          fakeRootCommands = ''
            mkdir -p /usr
            mkdir -p /tmp
            ln -s /bin /usr/bin
            mkdir -p /root
            chmod 700 /root
            echo "root:x:0:0:root:/root:/bin/sh" > /etc/passwd
            echo "root:x:0:" > /etc/group
          '';

          enableFakechroot = true;
        };
      in
      {
        checks = {
          inherit bk;
        };

        packages.default = bk;
        packages.containerImage = dockerImage;

        apps.default = flake-utils.lib.mkApp {
          drv = bk;
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};

          # Additional dev-shell environment variables can be set directly
          # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

          # Extra inputs can be added here; cargo and rustc are provided by default.
          packages = [
            # pkgs.ripgrep
          ];
        };
      }
    );
}
