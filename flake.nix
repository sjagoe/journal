# SPDX-License-Identifier: Unlicense
{
  description = "Age-encrypted journaling";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    { nixpkgs, crane, rust-overlay, flake-utils, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      eachSystem = flake-utils.lib.eachSystem systems;
    in
      eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;

          buildPlatformSuffix = pkgs.lib.strings.toLower pkgs.pkgsBuildHost.stdenv.hostPlatform.rust.cargoEnvVarTarget;

          toolchain =
            rust-overlay.packages.${system}.default;
            # with fenix.packages.${system};
            # combine [
            #   default.toolchain
            #   stable.rust-src
            #   targets.x86_64-pc-windows-gnu.latest.rust-std
            # ];

          craneLib = (crane.mkLib pkgs).overrideToolchain (p: toolchain);

          src = craneLib.cleanCargoSource ./.;

          # Common arguments can be set here to avoid repeating them later
          commonArgs =
            let
              cargoArgs = craneLib.crateNameFromCargoToml {
                inherit src;
              };
            in
              {
                inherit (cargoArgs) version;
                inherit src;
                strictDeps = true;

                buildInputs = [
                  pkgs.libgpg-error.dev
                  pkgs.gpgme.dev
                ];

                nativeBuildInputs = [
                  pkgs.pkg-config
                  pkgs.autoPatchelfHook
                  # Add additional build inputs here
                ];

                # Additional environment variables can be set directly
                # MY_CUSTOM_VAR = "some value";
              };

          # Build *just* the cargo dependencies (of the entire workspace),
          # so we can reuse all of that work (e.g. via cachix) when running in CI
          # It is *highly* recommended to use something like cargo-hakari to avoid
          # cache misses when building individual top-level-crates
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          journal = craneLib.buildPackage commonArgs // {
            inherit cargoArtifacts;
            inherit src;
            inherit (craneLib.crateNameFromCargoToml {
              inherit src;
            }) version;
            # NB: we disable tests since we'll run them all via cargo-nextest
            doCheck = false;
          };
        in
          {
            packages = {
              default = journal;
              inherit journal;
            };

            devShells.default = craneLib.devShell {
              # Additional dev-shell environment variables can be set directly
              # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

              # Extra inputs can be added here; cargo and rustc are provided by default.
              packages = [
                pkgs.rust-analyzer-unwrapped
                pkgs.pkg-config
                pkgs.libgpg-error
                pkgs.gpgme
              ];
            };
          });
}
