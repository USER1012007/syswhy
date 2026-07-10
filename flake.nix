{
  description = "syswhy development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        rustToolchain = with pkgs; [
          rustc
          cargo
          clippy
          rustfmt
          rust-analyzer
        ];

        nativeTools = with pkgs; [
          stdenv.cc
          pkg-config
          nix
        ];

        checkTools = with pkgs; [
          stdenv.cc
          pkg-config
        ];

        mkCargoCheck =
          name: command:
          pkgs.runCommand name { nativeBuildInputs = rustToolchain ++ checkTools; } ''
            cp -r ${self} source
            chmod -R u+w source
            cd source
            export CARGO_TARGET_DIR="$TMPDIR/target"
            ${command}
            touch $out
          '';
      in
      {
        devShells.default = pkgs.mkShell {
          packages = rustToolchain ++ nativeTools;

          RUST_BACKTRACE = "1";
        };

        checks = {
          fmt = mkCargoCheck "syswhy-fmt-check" "cargo fmt --check";
          test = mkCargoCheck "syswhy-test" "cargo test --locked";
          clippy = mkCargoCheck "syswhy-clippy" "cargo clippy --locked --all-targets --all-features -- -D warnings";
        };
      }
    );
}
