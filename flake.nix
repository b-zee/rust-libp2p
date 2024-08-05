{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, naersk, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        toolchain = pkgs.rust-bin.stable."1.80.0".default.override { targets = [ "x86_64-unknown-linux-musl" ]; };
        naersk' = pkgs.callPackage naersk { cargo = toolchain; rustc = toolchain; clippy = toolchain; };
      in
      rec {
        defaultPackage = naersk'.buildPackage {
          src = ./.;
        };

        packages.static =
          naersk'.buildPackage {
            src = ./.;
            doCheck = false;
            nativeBuildInputs = [ pkgs.pkgsStatic.stdenv.cc ];

            CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
            CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
          };
      }
    );
}
