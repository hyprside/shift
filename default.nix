{
  pkgs ? import <nixos-unstable> {},
  ...
}:
  let lib = pkgs.lib; in
pkgs.mkShell rec {
  packages = with pkgs; [
    mesa
    libGL
    cmake
    glfw
    pkg-config
    libgbm
    xorg.libX11
    xorg.libXrandr
    xorg.libXinerama
    xorg.libXcursor
    xorg.libXi
    python3
    ninja
    fontconfig
    freetype

    rustc
    rustfmt
    cargo
    clippy
    rust-analyzer
    libinput
    libxkbcommon
    cairo
    hyprcursor
  ];
  LD_LIBRARY_PATH = lib.makeLibraryPath packages;
}
