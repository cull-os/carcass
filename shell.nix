with import <nixpkgs> {};

mkShell {
  packages = [
    cargo-fuzz
  ];

  LD_LIBRARY_PATH = "${stdenv.cc.cc.lib}/lib";

  shellHook = ''
    root=$(git rev-parse --show-toplevel 2>/dev/null || echo ".")
    export PATH=$PATH:$root/target/debug:$root/target/release

    if command -v crash >/dev/null 2>&1; then
      crash
      exit $?
    fi
  '';
}
