{ inputs, pkgs, ... }:
{
  imports = [ inputs.tokenmaxxing.homeManagerModules.default ];

  home.packages = [
    pkgs.claude-code
    pkgs.codex
    pkgs.cursor-cli
  ];

  programs.tokenmaxxing.enable = true;
  programs.tokenmaxxing.package = inputs.tokenmaxxing.packages.${pkgs.stdenv.hostPlatform.system}.default;
}
