{ lib, pkgs, ... }:
let
  zsh = ../../assets/dotfiles/zsh;
in
{
  home.stateVersion = "26.05";

  home.packages = [
    pkgs.btop
    pkgs.bun
    pkgs.dotenvx
    pkgs.inngest
    (pkgs.callPackage ../pkgs/resend-cli.nix { })
  ];

  home.activation.mintGlobal = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    if [ ! -x "$HOME/.bun/bin/mint" ]; then
      bun="$HOME/.bun/bin/bun"
      [ -x "$bun" ] || bun="/etc/profiles/per-user/$USER/bin/bun"
      [ -x "$bun" ] || bun="$HOME/.nix-profile/bin/bun"
      if [ -x "$bun" ]; then
        run "$bun" add --global mint
      fi
    fi
  '';

  home.file.".hushlogin".text = "";

  home.file.".zshenv".source = zsh + "/.zshenv";
  home.file.".zshrc".source = zsh + "/.zshrc";
  home.file.".zprofile".source = zsh + "/.zprofile";
  home.file.".zlogin".source = zsh + "/.zlogin";

  home.file.".config/zsh/lib".source = zsh + "/lib";
  home.file.".config/zsh/rc".source = zsh + "/rc";
  home.file.".config/zsh/bin".source = zsh + "/bin";

  home.file.".claude/CLAUDE.md".source = ../../AGENTS.md;
  home.file.".codex/AGENTS.md".source = ../../AGENTS.md;
}
