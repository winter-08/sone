{
  description = "SONE — native Linux client for TIDAL (lossless, bit-perfect)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  # Builds a crate's dependencies as their own derivation (rune-app/package.nix).
  inputs.crane.url = "github:ipetkov/crane";

  outputs =
    { self, nixpkgs, crane }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      packages.${system} = {
        # The app (rune-app/), installed as RONE.
        rone = pkgs.callPackage ./rune-app/package.nix {
          src = ./.;
          craneLib = crane.mkLib pkgs;
        };
        default = self.packages.${system}.rone;
      };

      apps.${system}.default = {
        type = "app";
        program = "${self.packages.${system}.rone}/bin/rone";
      };

      # The build shell (lld, sccache, the libraries the app opens at run time).
      devShells.${system}.default = import ./rune-app/shell.nix { inherit pkgs; };

      formatter.${system} = pkgs.nixfmt;

      checks.${system}.build = self.packages.${system}.rone;
    };
}
