{ config, inputs, lib, ... }: {
  pkgsOverlays = [(self: _: {
    crane = (inputs.crane.mkLib self).overrideToolchain self.fenix.complete.toolchain;
  })];

  projectTypes = [ "cargo" ];

  perSystem = { system, pkgs, ... }: let
    projects = lib.filterAttrs (_: projectConfig: projectConfig.type == "cargo") config.projects;
  in
    lib.foldl' lib.recursiveUpdate {} <| lib.attrValues <| lib.flip lib.mapAttrs projects (projectName: projectConfig: let
    src = pkgs.crane.cleanCargoSource projectConfig.source;

    cargoArguments = {
      inherit src;

      strictDeps = true;
    };

    cargoArtifacts = pkgs.crane.buildDepsOnly cargoArguments;

    packages = projectConfig.packages
      |> map (packageName: lib.nameValuePair "${projectName}${lib.optionalString (packageName != projectName) "-${packageName}"}" <| pkgs.crane.buildPackage <| cargoArguments // {
        inherit cargoArtifacts;

        pname          =              packageName;
        cargoExtraArgs = "--package ${packageName}";

        doCheck = false;
      })
      |> lib.listToAttrs;
  in {
    inherit packages;

    devShells.${projectName} = pkgs.crane.devShell {
      packages = projectConfig.shell.packages ++ [
        # Better tests.
        pkgs.cargo-nextest

        # TOML formatting.
        pkgs.taplo

        # Fuzzing.
        pkgs.cargo-fuzz
      ];

      env.CLIPPY_CONF_DIR = pkgs.writeTextDir "clippy.toml" <| lib.readFile ../.clippy.toml;

      shellHook = ''
        # So we can do `{bin}` instead of `./target/debug/{bin}`
        root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
        export PATH="$PATH":"$root/target/debug"

        ${projectConfig.shell.hook}
      '';
    };

    checks = lib.mapAttrs' (name: lib.nameValuePair "package-${name}") packages // {
      "${projectName}-doctest" = pkgs.crane.cargoDocTest (cargoArguments // {
        inherit cargoArtifacts;
      });

      "${projectName}-nextest" = pkgs.crane.cargoNextest (cargoArguments // {
        inherit cargoArtifacts;
      });

      "${projectName}-clippy" = pkgs.crane.cargoClippy (cargoArguments // {
        inherit cargoArtifacts;

        env.CLIPPY_CONF_DIR = pkgs.writeTextDir "clippy.toml" <| lib.readFile ../.clippy.toml;

        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      });

      "${projectName}-doc" = pkgs.crane.cargoDoc (cargoArguments // {
        inherit cargoArtifacts;
      });

      "${projectName}-fmt" = pkgs.crane.cargoFmt {
        inherit src;

        rustFmtExtraArgs = "--config-path ${../.rustfmt.toml}";
      };

      "${projectName}-toml-fmt" = pkgs.crane.taploFmt {
        src = lib.sources.sourceFilesBySuffices src [ ".toml" ];

        taploExtraArgs = "--config ${../.taplo.toml}";
      };

      "${projectName}-audit" = pkgs.crane.cargoAudit {
        inherit (inputs) advisory-db;
        inherit src;
      };

      "${projectName}-deny" = pkgs.crane.cargoDeny {
        inherit src;

        cargoDenyChecks = "--config ${../.deny.toml}";
      };
    };
  });
}
