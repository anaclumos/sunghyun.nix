{ pkgs, inputs, ... }:
{
  fonts.fontconfig.enable = true;

  home.packages = [ inputs.sunghyun-sans.packages.${pkgs.stdenv.hostPlatform.system}.default ];
}
