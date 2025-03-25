{ inputs, config, ... }: let
  inherit (inputs.nixpkgs) lib;
  inherit (lib) types;

  overlayType = lib.mkOptionType {
    name        = "overlay";
    description = "overlay";
    check       = lib.isFunction;
    merge       = lib.mergeOneOption;
  };
in {
  options.pkgsOverlays = lib.mkOption {
    type        = types.listOf overlayType;
    default     = [];
    description = ''
      List of overlays to apply to pkgs.
    '';
  };

  config.perSystem = { system, ... }: {
    config._module.args.pkgs = import inputs.nixpkgs {
      inherit system;

      overlays = let
        localOverlays = config.pkgsOverlays;

        outerOverlays = lib.removeAttrs inputs [ "self" ]
          |> lib.attrValues
          |> lib.filter (flake: flake ? overlays.default)
          |> map        (flake: flake . overlays.default);
      in localOverlays ++ outerOverlays;
    };
  };

  options.libOverlays = lib.mkOption {
    type        = types.listOf overlayType;
    default     = [];
    description = ''
      List of overlays to apply to lib.
    '';
  };

  config._module.args.lib = lib.foldl'
    (acc: next: acc.extend next)
    lib
    config.libOverlays;
}
