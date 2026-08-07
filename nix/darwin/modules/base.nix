# Shared nix-darwin base for owner Macs.
{ pkgs, ... }:
{
  nixpkgs.hostPlatform = "aarch64-darwin";

  # Set once at first install (current default per manual); never bump casually.
  system.stateVersion = 7;

  # Required for homebrew / user-scoped activation (nix-darwin primaryUser migration).
  system.primaryUser = "sc";

  users.users.sc = {
    name = "sc";
    home = "/Users/sc";
  };

  # Determinate / upstream multi-user Nix already owns the daemon on many hosts.
  # Keep false so nix-darwin does not fight an existing installer. Set true only
  # when this flake should manage nix.conf itself.
  nix.enable = false;

  programs.zsh.enable = true;

  # tmux from nixpkgs (not only brew): privileged automation phases run inside
  # one persistent tmux session so a single sudo authentication covers the run.
  environment.systemPackages = [ pkgs.tmux ];

  # Touch ID (and Apple Watch) for sudo, with pam_reattach so it also works
  # inside tmux/screen. Kills the password-prompt pain declaratively; typed
  # passwords remain the fallback when biometrics are unavailable.
  #
  # tty_tickets stays at the macOS default (enabled) on purpose: per-tty
  # timestamps are the safer choice, and the owner-mandated pattern (2026-08-07)
  # is one persistent tmux pane per privileged run — a single `sudo -v` there
  # covers the whole run, so disabling tty_tickets buys nothing.
  security.pam.services.sudo_local = {
    enable = true;
    touchIdAuth = true;
    watchIdAuth = true;
    reattach = true;
  };
}
