{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShellNoCC {
  packages = (with pkgs; [
    cargo
    clippy
    gnumake
    nodejs_24
    pnpm
    pkg-config
    rustc
    rustfmt
  ]) ++ pkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
    gtk3
    webkitgtk_4_1
  ]);
}
