let
  pkgs = import <nixpkgs> {};
in
pkgs.stdenv.mkDerivation {
  name = "rust-env";
  nativeBuildInputs = with pkgs; [
    rustc cargo rustfmt rls pkgconfig
  ];
  buildInputs = with pkgs; [
    openssl
    wayland
    x11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
  ];

  RUST_BACKTRACE = 1;
}
