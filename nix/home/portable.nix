# Portable Home Manager user layer (no darwin-only options).
#
# Attached by both the darwin host (nix/darwin/modules/home.nix) and the
# standalone Linux output (homeConfigurations."sc@linux"). Nothing in this
# file may reference nix-darwin options, launchd, homebrew, or macOS paths.
{ config, ... }:
let
  configs = "${config.home.homeDirectory}/Developer/configs";
  # Out-of-store symlink: HM owns the wiring, the configs working copy owns
  # the content. Edits in ~/Developer/configs apply immediately (no second
  # clone to pull, no drift channel).
  zshDot = name: config.lib.file.mkOutOfStoreSymlink "${configs}/zsh/${name}";
in
{
  # Set once at first activation; never change.
  home.stateVersion = "26.05";

  home.file.".hushlogin".text = "";

  # Shell rc content stays with the dotfiles repo (anaclumos/configs); HM only
  # pins the symlinks so they cannot silently point anywhere else.
  # Decision 2026-08-08: the single canonical clone is ~/Developer/configs
  # (install.sh ensures it exists on fresh machines). The former managed clone
  # under ~/.local/share/sunghyun-os/dotfiles is retired.
  # Do NOT enable programs.zsh: it would generate ~/.zshrc and take over
  # content ownership.
  home.file.".zshrc".source = zshDot ".zshrc";
  home.file.".zshenv".source = zshDot ".zshenv";
  home.file.".zprofile".source = zshDot ".zprofile";
  home.file.".zlogin".source = zshDot ".zlogin";
}
