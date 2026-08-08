{ inputs, pkgs, ... }:
{
  imports = [ inputs.tokenmaxxing.homeManagerModules.default ];

  # macOS gets these through nix-darwin `homebrew` instead; installing them
  # twice would leave two binaries fighting over PATH order.
  home.packages = [
    pkgs.claude-code
    pkgs.codex
    pkgs.cursor-cli
  ];

  # macOS gets tokenmaxxing through the darwin module in flake.nix; here the
  # standalone Home Manager pkgs has no overlay, so the package is explicit.
  programs.tokenmaxxing.enable = true;
  programs.tokenmaxxing.package = inputs.tokenmaxxing.packages.${pkgs.system}.default;
}
