{ config, lib, ... }:
let
  # Apple's media top row is the base state, so Karabiner has to invert only
  # the few keys that must stay function keys; the opposite base state would
  # need a manipulator for every other key in the row.
  standardFunctionKeys = false;
  primaryUser = config.system.primaryUser;
in
{
  system.defaults = {
    NSGlobalDomain = {
      "com.apple.keyboard.fnState" = standardFunctionKeys;
    };
    # 0 = when space allows, 1 = always, 2 = never.
    menuExtraClock.ShowDate = 2;
    CustomUserPreferences = {
      "com.apple.systemuiserver" = {
        "NSStatusItem VisibleCC com.apple.menuextra.TimeMachine" = false;
        "NSStatusItem Visible com.apple.menuextra.TimeMachine" = false;
        menuExtras = [ ];
      };
    };
  };

  # `defaults write` only updates .GlobalPreferences; IOHIDSystem reads
  # com.apple.keyboard.fnState into its HIDParameters at login, so a running
  # session keeps the old top row until something converges it here.
  system.activationScripts.postActivation.text = lib.mkAfter ''
    echo >&2 "sunghyun: converging top-row fn behaviour into IOHIDSystem"
    if ! launchctl asuser "$(id -u -- ${lib.escapeShellArg primaryUser})" \
      sudo --user=${lib.escapeShellArg primaryUser} -- \
      /usr/local/bin/sunghyun fn-state apply \
      --standard-function-keys ${lib.boolToString standardFunctionKeys}; then
      echo >&2 "sunghyun: WARNING could not set HIDFKeyMode; the top row converges at next login"
    fi
  '';
}
