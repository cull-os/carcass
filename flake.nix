{
  description = "A reproducible contextful-expression language";

  nixConfig = {
    extra-substituters = [
      "https://cache.garnix.io/"
      "https://nix-community.cachix.org/"
    ];

    extra-trusted-public-keys = [
      "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };

  inputs = {
    systems.url = "github:nix-systems/default";

    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    fenix = {
      url = "github:nix-community/fenix";

      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, systems, nixpkgs, fenix }: let
    inherit (nixpkgs.lib) genAttrs makeLibraryPath optionals;

    eachPkgs = callback: genAttrs (import systems) (system: callback <| import nixpkgs {
      inherit system;

      overlays = [
        fenix.overlays.default
      ];
    });
  in {
    devShells = eachPkgs (pkgs: let
      cab = pkgs.mkShell {
        packages = [
          # You will need a nightly Rust compiler.
          pkgs.fenix.complete.toolchain

          # TOML formatting.
          pkgs.taplo

          # Fuzzing.
          pkgs.cargo-fuzz
        ];

        env.LD_LIBRARY_PATH = makeLibraryPath <| optionals pkgs.stdenv.targetPlatform.isLinux [
          pkgs.stdenv.cc.cc.lib
        ];

        shellHook = ''
          # So we can do `{bin}` instead of `./target/{optimization}/{bin}`
          root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
          export PATH=$PATH:$root/target/debug:$root/target/release
        '';
      };
    in {
      inherit cab;
      default = cab;
    });
  };
}
