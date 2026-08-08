{ pkgs, ... }:
{
  # macOS gets these through nix-darwin `homebrew` instead; installing them
  # twice would leave two binaries fighting over PATH order.
  home.packages = [
    pkgs.claude-code
    pkgs.codex
    pkgs.cursor-cli
  ];
}
