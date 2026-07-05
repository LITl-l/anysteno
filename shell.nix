# Classic nix-shell entry point (works without flake network fetches).
#   nix-shell        -> dev shell with Rust + GUI/input system libs
# Prefer this on NixOS where <nixpkgs> is a local channel.
{ pkgs ? import <nixpkgs> { } }:

let
  # System libraries for eframe/egui (GL, X11/wayland), rdev (X11 XInput/XTest),
  # and enigo (X11 XTest / libxdo) on Linux.
  linuxLibs = with pkgs; [
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    xorg.libXtst
    xorg.libxcb
    libxkbcommon
    wayland
    libGL
    fontconfig
    freetype
    xdotool
  ];

  buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux linuxLibs;
in
pkgs.mkShell {
  inherit buildInputs;
  nativeBuildInputs = with pkgs; [ rustc cargo rustfmt clippy pkg-config ];
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
  shellHook = ''echo "anysteno dev shell — $(rustc --version)"'';
}
