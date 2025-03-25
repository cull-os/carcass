{ config, lib, ... }: let
  inherit (lib) types;
in {
  options.projectTypes = lib.mkOption {
    type        = types.listOf types.str;
    default     = [];
    description = ''
      List of supported project types
    ''; 
  };

  options.projects = lib.mkOption {
    type = types.attrsOf <| types.submodule {
      options.source = lib.mkOption {
        type = types.path;
        description = ''
          The source of the project.
        '';
      };

      options.type = lib.mkOption {
        type        = types.enum config.projectTypes;
        description = ''
          The type of the project.
        '';
      };

      options.packages = lib.mkOption {
        type        = types.listOf types.str;
        description = ''
          Package names that should be handled by the proeject type handler.
        '';
      };

      options.shell.packages = lib.mkOption {
        type        = types.listOf types.package;
        default     = [];
        description = ''
          Extra shell packages.
        '';
      };

      options.shell.hook = lib.mkOption {
        type        = types.lines;
        default     = "";
        description = ''
          Extra shell hook.
        '';
      };
    };
  };
}
