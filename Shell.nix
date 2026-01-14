{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pkg-config
    openssl
    openssl.dev   # ← this provides the headers (libssl-dev equivalent)
  ];

  # Important: helps rust find openssl
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
  OPENSSL_DIR = "${pkgs.openssl}";
  OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
  OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";

  # Optional but very useful
  buildInputs = with pkgs; [
    rustc
    cargo
    rust-analyzer  # for editor support if you want
  ];
}