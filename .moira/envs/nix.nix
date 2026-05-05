{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = [ pkgs.nix pkgs.skopeo pkgs.jq pkgs.fd ];
}
