{ lib, ... }:
let
  assets = ../../../assets;
  hammerspoon = import ../../hammerspoon.nix { inherit lib; };
in
{
  imports = [ ../../home/portable.nix ];

  home.file.".hammerspoon/init.lua".text = hammerspoon.initLua;
  home.file.".config/karabiner/karabiner.json".source = assets + "/karabiner.json";
  home.file.".config/ghostty/config".source = assets + "/dotfiles/ghostty/config";
}
