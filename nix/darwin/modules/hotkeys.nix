{
  config,
  lib,
  pkgs,
  self,
  ...
}:
let
  primaryUser = config.system.primaryUser;
  sunghyun = lib.getExe self.packages.${pkgs.stdenv.hostPlatform.system}.sunghyun;
in
{
  system.activationScripts.postActivation.text = lib.mkAfter ''
    echo >&2 "sunghyun: freeing chords reserved for apps (⌘⇧Space → 1Password)"
    if ! launchctl asuser "$(id -u -- ${lib.escapeShellArg primaryUser})" \
      sudo --user=${lib.escapeShellArg primaryUser} -- \
      ${sunghyun} hotkeys apply; then
      echo >&2 "sunghyun: WARNING could not free the reserved chords; they converge at next login"
    fi
  '';
}
