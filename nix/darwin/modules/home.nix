{ config, lib, ... }:
let
  home = config.home.homeDirectory;
  subst = text: builtins.replaceStrings [ "HOME_DIR_PLACEHOLDER" ] [ home ] text;
  assets = ../../../assets;
  hammerspoon = import ../../hammerspoon.nix { inherit lib; };
in
{
  imports = [ ../../home/portable.nix ];

  home.file.".hammerspoon/init.lua".text = hammerspoon.initLua;

  # Karabiner-Elements live-reloads this file and refuses GUI edits against it,
  # which is the point: the repo is the only source of truth.
  home.file.".config/karabiner/karabiner.json".source = assets + "/karabiner.json";

  home.file.".config/sunghyun/kanata.kbd".text = subst (
    builtins.readFile (assets + "/kanata.kbd")
  );
  home.file.".config/sunghyun/kanata-passthrough.kbd".source =
    assets + "/kanata-passthrough.kbd";
  home.file.".config/sunghyun/run-sunghyun.sh" = {
    text = subst (builtins.readFile (assets + "/run-sunghyun.sh"));
    executable = true;
  };
  home.file.".config/ghostty/config".source = assets + "/dotfiles/ghostty/config";

  home.file.".config/sunghyun/Brewfile".source = assets + "/Brewfile";
  home.file.".config/sunghyun/manifest.toml".source = assets + "/manifest.toml";
}
