{
  description = "The monorepository of the Cull Organization.";

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
    systems.url     = "github:nix-systems/default";
    flake-parts.url = "github:hercules-ci/flake-parts";

    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";

      inputs.nixpkgs.follows = "nixpkgs";
    };

    advisory-db = {
      url   = "github:rustsec/advisory-db";
      flake = false;
    };

    devshell = {
      url = "github:numtide/devshell";

      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ { systems, flake-parts, ... }: flake-parts.lib.mkFlake { inherit inputs; } ({ lib, ... }: {
    systems = import systems;

    perSystem = { system, pkgs, ... }: {
      _module.args.pkgs = import inputs.nixpkgs {
        inherit system;

        overlays = lib.attrValues inputs
          |> lib.filter (flake: flake ? overlays.default)
          |> map        (flake: flake.overlays.default);
      };

      _module.args.cargoLib = let
        cargoLib = inputs.crane.mkLib pkgs;
      in cargoLib.overrideToolchain pkgs.fenix.complete.toolchain;
    };

    imports = let
      localModules = builtins.readDir ./.
        |> lib.filterAttrs (_: type: type == "directory")
        |> lib.attrNames
        |> map (directory: ./${directory}/default.nix)
        |> lib.filter lib.pathExists;

      outerModules = lib.removeAttrs inputs [ "self" ]
        |> lib.attrValues 
        |> lib.filter (flake: flake ? flakeModule)
        |> map        (flake: flake.flakeModule);
    in localModules ++ outerModules;
  });
}
