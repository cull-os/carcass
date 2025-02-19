{ lib, ... }: {
  perSystem = { inputs', system, pkgs, cargoLib, ... }: {
    devshells.cab = {
      packages = [
        # You will need a nightly Rust compiler.
        pkgs.fenix.complete.toolchain

        # TOML formatting.
        pkgs.taplo

        # Fuzzing.
        pkgs.cargo-fuzz
      ];

      env = [
        (lib.mkIf pkgs.stdenv.hostPlatform.isLinux {
          name  = "LD_LIBRARY_PATH";
          value = lib.makeLibraryPath [
            pkgs.stdenv.cc.cc.lib
          ];
        })

        {
          name   = "PATH";
          prefix = "target/debug";
        }
      ];
    };

    checks = let
      src = cargoLib.cleanCargoSource ./.;

      cargoArguments = {
        inherit src;

        strictDeps = true;
      };

      cargoArtifacts = cargoLib.buildDepsOnly cargoArguments;

      cab = cargoLib.buildPackage (cargoArguments // {
        inherit cargoArtifacts;

        doCheck = false;
      });
    in {
      inherit cab;

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
      };

      cab-toml-fmt = cargoLib.taploFmt {
        src = lib.sources.sourceFilesBySuffices src [ ".toml" ];

        # TODO: Configure `cargoLib` centrally to set this.
        taploExtraArgs = "--config ${../.taplo.toml}";
      };

      cab-audit = cargoLib.cargoAudit {
        inherit (inputs') advisory-db;
        inherit src;
      };

      cab-deny = cargoLib.cargoDeny {
        inherit src;
      };
    };
  };
}
