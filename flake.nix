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
    flake-parts.url = "github:hercules-ci/flake-parts";

    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    # RUST
    crane.url   = "github:ipetkov/crane";
    fenix       = { url = "github:nix-community/fenix"; inputs.nixpkgs.follows = "nixpkgs"; };
    advisory-db = { url = "github:rustsec/advisory-db"; flake = false; };
  };

  outputs = inputs @ { systems, flake-parts, ... }: flake-parts.lib.mkFlake { inherit inputs; } ({ lib, ... }: {
    systems = import systems;

    imports = let
      localModules = lib.filesystem.listFilesRecursive ./.
        |> lib.filter (absolutePath: let
            path = builtins.baseNameOf absolutePath;
          in lib.hasPrefix "__" path && lib.hasSuffix "__" (lib.removeSuffix ".nix" path));

      outerModules = lib.removeAttrs inputs [ "self" ]
        |> lib.attrValues 
        |> lib.filter (flake: flake ? flakeModule)
        |> map        (flake: flake . flakeModule);
    in localModules ++ outerModules;
  });
}
