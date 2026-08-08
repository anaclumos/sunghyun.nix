{ pkgs, inputs, ... }:
{
  nixpkgs.hostPlatform = "aarch64-darwin";

  nixpkgs.config.allowUnfree = true;

  system.stateVersion = 7;

  system.primaryUser = "sc";

  users.users.sc = {
    name = "sc";
    home = "/Users/sc";
  };

  nix.enable = false;

  system.tools.darwin-uninstaller.enable = false;

  programs.zsh.enable = true;

  environment.systemPackages = [ pkgs.tmux ];

  fonts.packages = [ inputs.sunghyun-sans.packages.${pkgs.stdenv.hostPlatform.system}.default ];

  security.pam.services.sudo_local = {
    enable = true;
    touchIdAuth = true;
    watchIdAuth = true;
    reattach = true;
  };
}
