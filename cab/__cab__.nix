{
  projects.cab = {
    source = ./.;
    type   = "cargo";

    packages = [ "cab" "cab-task" ];
  };
}
