{
  description = "Build a cargo findbar project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      fenix,
      ...
    }:
    let
      eachSystem = flake-utils.lib.eachDefaultSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          inherit (pkgs) lib;

          # Get the fenix packages for the current system
          fenixPkgs = fenix.packages.${system};

          # Combine the components you need into a single toolchain
          rustToolchain = fenixPkgs.stable.withComponents [
            "cargo"
            "clippy"
            "rust-src"
            "rustc"
            "rustfmt"
            "rust-analyzer"
          ];

          # Tell crane to use your combined fenix toolchain
          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          unfilteredRoot = ./.; # The original, unfiltered source
          src = craneLib.cleanCargoSource unfilteredRoot;

          # Common arguments can be set here to avoid repeating them later
          commonArgs = {
            inherit src;
            strictDeps = true;

            nativeBuildInputs = [
              pkgs.pkg-config
            ];

            buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
              pkgs.apple-sdk_15
            ];
          };

          # Build *just* the cargo dependencies, so we can reuse
          # all of that work (e.g. via cachix) when running in CI
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          # Build the actual crate itself, reusing the dependency
          # artifacts from above.
          findbar-crate = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );

        in
        {
          checks = {
            # Build the crate as part of `nix flake check` for convenience
            inherit findbar-crate;

            # Add clippy and fmt checks that automatically use the fenix toolchain
            findbar-crate-clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              }
            );

            findbar-crate-fmt = craneLib.cargoFmt {
              inherit src;
            };
          };

          packages = {
            default = findbar-crate;
            findbar = findbar-crate;
          };

          devShells.default = craneLib.devShell {
            # This is so that rust_analyzer in zed works:
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

            packages = [
            ];

            shellHook =
              lib.optionalString pkgs.stdenv.hostPlatform.isDarwin
                ''
                  export DEVELOPER_DIR=$(env -u DEVELOPER_DIR /usr/bin/xcode-select -p)
                '';
          };
        }
      );
    in
    eachSystem
    // {
      darwinModules = {
        default = import ./modules/darwin.nix self;
        findbar = self.darwinModules.default;
      };

      homeManagerModules = {
        default = import ./modules/home-manager.nix self;
        findbar = self.homeManagerModules.default;
      };

      overlays.default = final: prev: {
        findbar = self.packages.${final.stdenv.hostPlatform.system}.default;
      };
    };
}
