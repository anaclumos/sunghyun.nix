{ ... }:
let
  zsh = ../../assets/dotfiles/zsh;
in
{
  home.stateVersion = "26.05";

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
}
