# Darwin-scoped Home Manager layer: keyboard engine assets + sunghyun
# runtime config. Portable user config lives in nix/home/portable.nix so the
# same user layer attaches to Linux hosts (homeConfigurations."sc@linux").
{ config, ... }:
let
  home = config.home.homeDirectory;
  subst =
    text: builtins.replaceStrings [ "HOME_DIR_PLACEHOLDER" ] [ home ] text;
  assets = ../../../assets;
in
{
  imports = [ ../../home/portable.nix ];

  # Primary keyboard engine (OUTCOMES.md a-e): Karabiner-Elements declarative
  # config. KE follows the symlink and live-reloads; GUI edits are refused,
  # which is intended (this file is the source of truth).
  home.file.".config/karabiner/karabiner.json".source = assets + "/karabiner.json";

  # Alternative engine (opt-in via `sunghyun kanata enable --safe` only).
  home.file.".config/sunghyun/kanata.kbd".text = subst (
    builtins.readFile (assets + "/kanata.kbd")
  );
  # Stage-0 identity config for `sunghyun kanata enable --safe`.
  home.file.".config/sunghyun/kanata-passthrough.kbd".source =
    assets + "/kanata-passthrough.kbd";
  home.file.".config/sunghyun/sunghyun.toml".source = assets + "/sunghyun.toml";
  home.file.".config/sunghyun/run-sunghyun.sh" = {
    text = subst (builtins.readFile (assets + "/run-sunghyun.sh"));
    executable = true;
  };
  home.file.".config/sunghyun/Brewfile".source = assets + "/Brewfile";
  home.file.".config/sunghyun/manifest.toml".source = assets + "/manifest.toml";
}
