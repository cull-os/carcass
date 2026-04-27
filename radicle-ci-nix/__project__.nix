{
  projects.radicle-ci-nix = {
    source = ./..;
    type   = "cargo";

    packages = [ "radicle-ci-nix" ];
  };
}
