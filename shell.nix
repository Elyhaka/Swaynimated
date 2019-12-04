let
  pkgs = import <nixpkgs> {};
in
pkgs.stdenv.mkDerivation {
  name = "rust-env";
  nativeBuildInputs = with pkgs; [
    rustc cargo rustfmt pkgconfig rls
  ];
  buildInputs = with pkgs; [
    openssl
  ];

  RUST_BACKTRACE = 1;
}
