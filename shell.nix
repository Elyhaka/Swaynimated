with import <nixpkgs> {};

stdenv.mkDerivation rec {
  name = "rust-env";

  nativeBuildInputs = with pkgs; [
    rustc
    cargo
    rustfmt
    rls
    cmake
    pkgconfig
    python3
  ];

  buildInputs = with pkgs; [
    wayland
    libGL
    x11
    libxkbcommon
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    xorg.libxcb
    xorg.libXxf86vm
    vulkan-loader
    shaderc
  ];

  LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";

  RUST_BACKTRACE = 1;
}
