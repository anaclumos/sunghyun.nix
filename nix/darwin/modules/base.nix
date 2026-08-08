{ pkgs, inputs, ... }:
{
  nixpkgs.hostPlatform = "aarch64-darwin";

  system.stateVersion = 7;

  system.primaryUser = "sc";

  users.users.sc = {
    name = "sc";
    home = "/Users/sc";
  };

  # Determinate already owns the daemon on these hosts; true would make
  # nix-darwin fight the existing installer over nix.conf.
  nix.enable = false;

  programs.zsh.enable = true;

  environment.systemPackages = [ pkgs.tmux ];

  fonts.packages = [ inputs.sunghyun-sans.packages.${pkgs.stdenv.hostPlatform.system}.default ];

  # `reattach` is what makes Touch ID work inside tmux, where privileged phases
  # run so one `sudo -v` covers the whole job.
  security.pam.services.sudo_local = {
    enable = true;
    touchIdAuth = true;
    watchIdAuth = true;
    reattach = true;
  };
}
