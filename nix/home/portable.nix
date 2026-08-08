{ pkgs, ... }:
let
  zsh = ../../assets/dotfiles/zsh;
in
{
  home.stateVersion = "26.05";

  # nixpkgs and not homebrew.brews: these entries cover macOS and the Linux
  # home configs alike. btop has no vendor self-update channel that would
  # need a writable install like cursor-cli does. dotenvx is only on the
  # third-party dotenvx/brew tap, and Homebrew on this macOS 27 host refuses
  # that prebuilt formula while /Applications/Xcode.app is still 26.6.
  home.packages = [
    pkgs.btop
    pkgs.dotenvx
  ];

  home.file.".hushlogin".text = "";

  # Never enable `programs.zsh`: it generates ~/.zshrc content and would take
  # ownership away from these vendored files.
  home.file.".zshenv".source = zsh + "/.zshenv";
  home.file.".zshrc".source = zsh + "/.zshrc";
  home.file.".zprofile".source = zsh + "/.zprofile";
  home.file.".zlogin".source = zsh + "/.zlogin";

  # ~/.zshenv hardcodes ZSH_CONFIG_HOME to ~/.config/zsh; these paths are
  # the other half of that contract.
  home.file.".config/zsh/lib".source = zsh + "/lib";
  home.file.".config/zsh/rc".source = zsh + "/rc";
  home.file.".config/zsh/bin".source = zsh + "/bin";

  # Global default instruction layer for the coding CLIs (owner 2026-08-08).
  # Both tools concatenate rather than override: this global file enters
  # context first and hand-written per-directory CLAUDE.md / AGENTS.md files
  # appear later and win conflicts, so only the global layer is managed here.
  # The repo root AGENTS.md is the one canonical guide; per-assistant copies
  # would drift.
  home.file.".claude/CLAUDE.md".source = ../../AGENTS.md;
  home.file.".codex/AGENTS.md".source = ../../AGENTS.md;
}
