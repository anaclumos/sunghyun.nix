# Spotlight / keyboard / menu bar defaults. Expand from configs system-settings
# inventory when keys are pinned. cua remains the fallback for GUI-only rows.
#
# Spotlight ⌘Space (symbolichotkeys id 64) restore still lives in
# `sunghyun post-switch` / `sunghyun spotlight restore` until the live
# plist shape is corroborated on the target macOS version.
#
# Time Machine menu bar: Control Center host key `TimeMachine=2` plus
# SystemUIServer VisibleCC are applied by the `sunghyun` menubar step (and
# post-switch). Cursor tray is app storage (`systemTrayEnabled`), not defaults.
{ config, lib, ... }:
let
  # OUTCOMES.md (o): Apple's media top row is the base state, so F1/F2/F3 and
  # F7-F12 fire brightness/Mission Control/media/volume bare and fn yields plain
  # function keys. Karabiner then inverts only F4/F5/F6, which is 6
  # manipulators; the opposite base state would need 18.
  standardFunctionKeys = false;
  primaryUser = config.system.primaryUser;
in
{
  system.defaults = {
    NSGlobalDomain = {
      # Intentionally minimal. Add only keys verified on the live machine.
      "com.apple.keyboard.fnState" = standardFunctionKeys;
    };
    # OUTCOMES.md (g): menu bar shows no date. Int enum: 0=when space allows,
    # 1=always, 2=never.
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
  # session (every switch, including the one install.sh performs) keeps the old
  # top row and Karabiner keeps reporting the old
  # system.use_fkeys_as_standard_function_keys. postActivation runs after the
  # userDefaults hook, so this is where the declared value becomes effective.
  # Non-fatal: a machine without a HID session must not fail the whole switch.
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
