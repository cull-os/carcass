with import <nixpkgs> {};

mkShell {
  packages = [
    cargo-fuzz
  ];

  LD_LIBRARY_PATH = "${stdenv.cc.cc.lib}/lib";

  shellHook = ''
    if command -v crash >/dev/null 2>&1; then
      crash
      exit $?
    fi
  '';
}
