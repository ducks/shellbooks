{ pkgs ? import <nixpkgs> {} }:

let
  fenix = import (fetchTarball "https://github.com/nix-community/fenix/archive/main.tar.gz") {};
  rustToolchain = fenix.complete.toolchain;
in
pkgs.mkShell {
  buildInputs = [
    # Rust toolchain (matches shelltrax/shellcast — fenix complete for 2024 edition)
    rustToolchain

    # Build tools
    pkgs.pkg-config

    # Audio (rodio/cpal pulls in alsa)
    pkgs.alsa-lib

    # HTTP/TLS for any future network sources
    pkgs.openssl

  ];

  RUST_BACKTRACE = "1";

  shellHook = ''
    echo ""
    echo "Shellbooks Development Environment"
    echo "==================================="
    echo "Rust: $(rustc --version)"
    echo "Cargo: $(cargo --version)"
    echo ""
  '';
}
