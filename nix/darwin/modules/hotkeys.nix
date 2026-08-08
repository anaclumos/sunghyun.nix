{ config, lib, ... }:
let
  primaryUser = config.system.primaryUser;
in
{
  # Not `system.defaults.CustomUserPreferences`: that writes whole keys, and
  # AppleSymbolicHotKeys is one key holding every system shortcut, so declaring
  # it here would silently drop the rest. `sunghyun hotkeys apply` rewrites the
  # single offending identifier and also drops it from the running window
  # server, which never re-reads the preference domain before the next login.
  system.activationScripts.postActivation.text = lib.mkAfter ''
    echo >&2 "sunghyun: freeing chords reserved for apps (⌘⇧Space → 1Password)"
    if ! launchctl asuser "$(id -u -- ${lib.escapeShellArg primaryUser})" \
      sudo --user=${lib.escapeShellArg primaryUser} -- \
      /usr/local/bin/sunghyun hotkeys apply; then
      echo >&2 "sunghyun: WARNING could not free the reserved chords; they converge at next login"
    fi
  '';
}
