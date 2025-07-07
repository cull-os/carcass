{
  description = "The Cull monorepository.";

  nixConfig = {
    builders-use-substitutes = true;
    flake-registry           = "";
    show-trace               = true;

    experimental-features = [
      "flakes"
      "nix-command"
      "pipe-operators"
    ];

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

    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";

      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    # RUST
    crane.url   = "github:ipetkov/crane";
    fenix       = { url = "github:nix-community/fenix"; inputs.nixpkgs.follows = "nixpkgs"; };
    advisory-db = { url = "github:rustsec/advisory-db"; flake = false; };
  };

  outputs = inputs @ { systems, flake-parts, ... }: flake-parts.lib.mkFlake { inherit inputs; } ({ lib, ... }: {
    systems = import systems;

    imports = let
      localModules = lib.filesystem.listFilesRecursive ./.
        |> lib.filter (pathAbsolute: let
            pathBase = builtins.baseNameOf     pathAbsolute;
            pathStem = lib.removeSuffix ".nix" pathBase;
          in pathStem != pathBase
          && lib.hasPrefix "__" pathStem
          && lib.hasSuffix "__" pathStem);

      outerModules = lib.removeAttrs inputs [ "self" ]
        |> lib.attrValues
        |> lib.catAttrs "flakeModule";
    in localModules ++ outerModules;
  });
}
