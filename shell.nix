with import <nixpkgs> {};

mkShell {
  packages = let
    fenix = import (fetchTarball "https://github.com/nix-community/fenix/archive/main.tar.gz") { inherit pkgs; };
  in [
    # You will need a nightly rust compiler.
    fenix.default.toolchain

    # Fuzzing.
    cargo-fuzz

    # Required by the rug crate.
    gnum4
    gnumake
  ];

  env.LD_LIBRARY_PATH = lib.makeLibraryPath ([
    # Required by the rug crate.
    gmp5
  ] ++ lib.optionals stdenv.targetPlatform.isLinux [
    stdenv.cc.cc.lib
  ]);

  shellHook = ''
    # So we can do `cab` instead of `./target/{optimization}/cab`
    root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
    export PATH=$PATH:$root/target/debug:$root/target/release
  '';
}
