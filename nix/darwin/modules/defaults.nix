{
  config,
  lib,
  pkgs,
  self,
  ...
}:
let
  # Apple's media top row is the base state, so Karabiner has to invert only
  # the few keys that must stay function keys; the opposite base state would
  # need a manipulator for every other key in the row.
  standardFunctionKeys = false;
  primaryUser = config.system.primaryUser;
  home = config.users.users.${primaryUser}.home;

  # Item info, label position and grid snapping live inside
  # com.apple.finder → DesktopViewSettings → IconViewSettings, and `defaults
  # write` has no key path, so the only faithful edit is read the dictionary,
  # change the three leaves, write it back. `defaults export` (not `defaults
  # read`) because the old-style text output loses every type: round-tripping
  # it turns iconSize and the booleans into strings. arrangeBy "grid" is Snap
  # to Grid; "none" is free placement and a sort key like "name" is the
  # stronger Sort By, which the owner did not ask for.
  desktopViewConverge = pkgs.writeShellScript "sunghyun-desktop-view" ''
    set -u
    PATH=/usr/bin:/bin:/usr/sbin:/sbin

    read_flag() {
      /usr/bin/defaults export com.apple.finder - 2>/dev/null \
        | /usr/bin/plutil -extract "DesktopViewSettings.IconViewSettings.$1" raw -o - - 2>/dev/null
    }

    if [ "$(read_flag showItemInfo)" = "true" ] && [ "$(read_flag labelOnBottom)" = "false" ] \
      && [ "$(read_flag arrangeBy)" = "grid" ]; then
      exit 0
    fi

    tmp="$(/usr/bin/mktemp -t sunghyun-desktop-view)" || exit 0
    trap 'rm -f "$tmp" "$tmp.sub"' EXIT

    if /usr/bin/defaults export com.apple.finder "$tmp" 2>/dev/null \
      && /usr/bin/plutil -extract DesktopViewSettings xml1 -o /dev/null "$tmp" 2>/dev/null; then
      for triple in "showItemInfo bool true" "labelOnBottom bool false" "arrangeBy string grid"; do
        key="''${triple%% *}"
        value="''${triple##* }"
        type="''${triple#* }"; type="''${type%% *}"
        /usr/libexec/PlistBuddy -c \
          "Set :DesktopViewSettings:IconViewSettings:$key $value" "$tmp" >/dev/null 2>&1 \
          || /usr/libexec/PlistBuddy -c \
            "Add :DesktopViewSettings:IconViewSettings:$key $type $value" "$tmp" >/dev/null 2>&1
      done
      /usr/bin/plutil -extract DesktopViewSettings xml1 -o "$tmp.sub" "$tmp" 2>/dev/null || exit 0
      /usr/bin/defaults write com.apple.finder DesktopViewSettings "$(/bin/cat "$tmp.sub")" || exit 0
    else
      # No Desktop view settings yet (fresh account): nothing to preserve, and
      # Finder fills the keys it is not given.
      /usr/bin/defaults write com.apple.finder DesktopViewSettings -dict-add IconViewSettings \
        '<dict><key>showItemInfo</key><true/><key>labelOnBottom</key><false/><key>arrangeBy</key><string>grid</string></dict>' || exit 0
    fi

    # cfprefsd hands Finder a cached copy; without the restart Finder can flush
    # its old copy back over this write.
    /usr/bin/killall Finder 2>/dev/null || true
    echo "sunghyun: desktop icons show item info with labels on the right, snapped to grid"
  '';

  # KakaoTalk ships en/ja/ko and must run Korean whatever the system language
  # is. Same shape as the Applications list in Language & Region, which writes
  # AppleLanguages into the app's own preference domain. KakaoTalk comes from
  # the App Store and is sandboxed, so that domain redirects into its container
  # and a write can be refused; never fail a switch over it.
  appLanguageConverge = pkgs.writeShellScript "sunghyun-app-language" ''
    set -u
    PATH=/usr/bin:/bin:/usr/sbin:/sbin

    if [ ! -d /Applications/KakaoTalk.app ]; then
      echo "sunghyun: KakaoTalk absent; language override skipped (converges after mas installs it)"
      exit 0
    fi
    if [ "$(/usr/bin/defaults read com.kakao.KakaoTalkMac AppleLanguages 2>/dev/null | /usr/bin/tr -d ' \n"()')" = "ko" ]; then
      exit 0
    fi
    if /usr/bin/defaults write com.kakao.KakaoTalkMac AppleLanguages -array ko 2>/dev/null; then
      echo "sunghyun: KakaoTalk set to Korean"
    else
      echo >&2 "sunghyun: WARNING KakaoTalk's sandbox container refused the language write; Language & Region owns it on this machine"
    fi
  '';
in
{
  system.defaults = {
    NSGlobalDomain = {
      "com.apple.keyboard.fnState" = standardFunctionKeys;
      # Three separate keys, all read by macOS: setting only some leaves
      # Settings metric while formatters stay imperial. AppleMetricUnits is an
      # integer enum at this nix-darwin revision, not a boolean.
      AppleTemperatureUnit = "Celsius";
      AppleMeasurementUnits = "Centimeters";
      AppleMetricUnits = 1;
      # Automatically hide and show the menu bar → Never. This is only the
      # desktop half; full-screen visibility is a separate key below, and on
      # macOS 15+ Control Center also stores the four-way enum.
      _HIHideMenuBar = false;
    };
    # Bare fn tap only; fn-as-modifier chords ride HIDFKeyMode, not this key.
    # A running session keeps the old tap behaviour until the next login.
    hitoolbox.AppleFnUsageType = "Show Emoji & Symbols";
    # 0 = when space allows, 1 = always, 2 = never.
    menuExtraClock.ShowDate = 2;
    finder.ShowHardDrivesOnDesktop = true;
    finder.ShowPathbar = true;
    finder.ShowStatusBar = true;
    # `[ ]` and not the default `null`: nix-darwin filters null options out of
    # the `defaults write` list, so null means unmanaged while an empty list is
    # written as an empty array. Finder and the Trash are Dock fixtures rather
    # than preferences, so they stay either way.
    dock = {
      persistent-apps = [ ];
      persistent-others = [ { folder = "${home}/Downloads"; } ];
      show-recents = false;
    };
    CustomUserPreferences = {
      # No typed option at the pinned nix-darwin rev. Never needs both halves
      # of the classic pair, otherwise Settings lands on In Full Screen Only.
      NSGlobalDomain = {
        AppleMenuBarVisibleInFullscreen = true;
      };
      # macOS 15+/Tahoe Settings UI reads this enum (0 Always, 1 On Desktop
      # Only, 2 In Full Screen Only, 3 Never). Typed system.defaults.controlcenter
      # writes the ByHost domain; this key lives in the non-ByHost plist.
      "com.apple.controlcenter" = {
        AutoHideMenuBarOption = 3;
      };
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
      ${lib.getExe self.packages.${pkgs.stdenv.hostPlatform.system}.sunghyun} \
      fn-state apply ${lib.boolToString standardFunctionKeys}; then
      echo >&2 "sunghyun: WARNING could not set HIDFKeyMode; the top row converges at next login"
    fi

    for script in ${desktopViewConverge} ${appLanguageConverge}; do
      launchctl asuser "$(id -u -- ${lib.escapeShellArg primaryUser})" \
        sudo --user=${lib.escapeShellArg primaryUser} -- "$script" || true
    done
  '';
}
