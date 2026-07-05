{
  description = "anysteno — cross-platform stenography typing app (any keyboard, any language)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # System libraries needed at build & run time on Linux for:
        #   - eframe/egui (windowing, GL, wayland/X11)
        #   - rdev       (global key capture: X11 XInput/XTest)
        #   - enigo      (text injection: X11 XTest / libxdo)
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

        nativeBuildInputs = with pkgs; [
          rustc
          cargo
          rustfmt
          clippy
          pkg-config
        ];

        libraryPath = pkgs.lib.makeLibraryPath buildInputs;
      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          # egui loads GL/X11 libs at runtime via dlopen; expose them.
          LD_LIBRARY_PATH = libraryPath;
          shellHook = ''
            echo "anysteno dev shell — rustc $(rustc --version)"
          '';
        };
      });
}
