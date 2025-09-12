{
  description = "Build a cargo project without extra checks";

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
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
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
          ];
          config = {
            Cmd = [ "/bin/bk" ];
            WorkingDir = "/app";
          };
        };

        bk-k8s = craneLib.buildPackage (
          commonArgs
          // {
            src = craneLib.cleanCargoSource ./k8s-operator;
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          }
        );

        bk-k8s-container = pkgs.dockerTools.buildLayeredImage {
          name = "bk-k8s";
          tag = "latest-${pkgs.stdenv.hostPlatform.linuxArch}";
          contents = [ bk-k8s ];
          config = {
            Cmd = [ "/bin/bk-k8s" ];
            WorkingDir = "/app";
          };
        };
      in
      {
        checks = {
          inherit bk;
        };

        packages.default = bk;
        packages.bk-k8s = bk-k8s;
        packages.bk-k8s-containerImage = bk-k8s-container;
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
