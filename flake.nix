{
  description = "SONE — native Linux client for TIDAL (lossless, bit-perfect)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  # Builds a crate's dependencies as their own derivation (rune-app/package.nix).
  inputs.crane.url = "github:ipetkov/crane";
  # Tracks rune-app/rust-toolchain.toml (nightly with parallel frontend) for Nix builds too.
  inputs.rust-overlay = {
    url = "github:oxalica/rust-overlay";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    { self, nixpkgs, crane, rust-overlay }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rune-app/rust-toolchain.toml;
      craneLib = ((crane.mkLib pkgs).overrideToolchain (_: rustToolchain)).overrideScope (_: _: {
        stdenvSelector = p: p.stdenvAdapters.useMoldLinker p.stdenv;
      });
    in
    {
      packages.${system} = {
        # The app (rune-app/), built with Rust from rust-toolchain.toml and linked with mold.
        sone = pkgs.callPackage ./rune-app/package.nix {
          src = ./.;
          inherit craneLib;
        };
        default = self.packages.${system}.sone;
      };

      apps.${system}.default = {
        type = "app";
        program = "${self.packages.${system}.sone}/bin/sone";
      };

      # The build shell (mold, sccache, the libraries the app links or opens at run time).
      devShells.${system}.default = import ./rune-app/shell.nix { inherit pkgs; };

      formatter.${system} = pkgs.nixfmt;

      checks.${system}.build = self.packages.${system}.sone;
    };
}
