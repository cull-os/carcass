{ inputs, lib, ... }: {
  perSystem = { system, pkgs, cargoLib, ... }: let
    src = cargoLib.cleanCargoSource ./.;

    cargoArguments = {
      inherit src;

      strictDeps = true;
    };

    cargoArtifacts = cargoLib.buildDepsOnly cargoArguments;

    cab = cargoLib.buildPackage (cargoArguments // {
      inherit cargoArtifacts;
        
      pname          =           "cab";
      cargoExtraArgs = "--package cab";

      doCheck = false;
    });

    cab-task = cargoLib.buildPackage (cargoArguments // {
      inherit cargoArtifacts;
        
      pname          =           "task";
      cargoExtraArgs = "--package task";

      doCheck = false;
    });
  in {
    devShells.cab = cargoLib.devShell {
      packages = [
        # You will need a nightly Rust compiler.
        pkgs.fenix.complete.toolchain

        # TOML formatting.
        pkgs.taplo

        # Fuzzing.
        pkgs.cargo-fuzz
      ];

      shellHook = ''
        # So we can do `{bin}` instead of `./target/{optimization}/{bin}`
        root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
        export PATH="$PATH":"$root/cab/target/debug"
      '';
    };

    packages = {
      inherit cab cab-task;
    };

    checks = {
      inherit cab cab-task;

      cab-test = cargoLib.cargoTest (cargoArguments // {
        inherit cargoArtifacts;
      });

      cab-clippy = cargoLib.cargoClippy (cargoArguments // {
        inherit cargoArtifacts;

        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      });

      cab-doc = cargoLib.cargoDoc (cargoArguments // {
        inherit cargoArtifacts;
      });

      cab-fmt = cargoLib.cargoFmt {
        inherit src;

        rustFmtExtraArgs = "--config-path ${../.rustfmt.toml}";
      };

      cab-toml-fmt = cargoLib.taploFmt {
        src = lib.sources.sourceFilesBySuffices src [ ".toml" ];

        # TODO: Configure `cargoLib` centrally to set this.
        taploExtraArgs = "--config ${../.taplo.toml}";
      };

      cab-audit = cargoLib.cargoAudit {
        inherit (inputs) advisory-db;
        inherit src;
      };

      # TODO: Find out why this doesn't work.
      # cab-deny = cargoLib.cargoDeny {
      #   inherit src;
      # };
    };
  };
}
