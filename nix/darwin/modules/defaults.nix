{
  config,
  lib,
  pkgs,
  self,
  ...
}:
let
  standardFunctionKeys = false;
  primaryUser = config.system.primaryUser;
  home = config.users.users.${primaryUser}.home;

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
      /usr/bin/defaults write com.apple.finder DesktopViewSettings -dict-add IconViewSettings \
        '<dict><key>showItemInfo</key><true/><key>labelOnBottom</key><false/><key>arrangeBy</key><string>grid</string></dict>' || exit 0
    fi

    /usr/bin/killall Finder 2>/dev/null || true
    echo "sunghyun: desktop icons show item info with labels on the right, snapped to grid"
  '';

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
      AppleTemperatureUnit = "Celsius";
      AppleMeasurementUnits = "Centimeters";
      AppleMetricUnits = 1;
      _HIHideMenuBar = false;
    };
    hitoolbox.AppleFnUsageType = "Show Emoji & Symbols";
    menuExtraClock.ShowDate = 2;
    finder.ShowHardDrivesOnDesktop = true;
    finder.ShowPathbar = true;
    finder.ShowStatusBar = true;
    dock = {
      persistent-apps = [ ];
      persistent-others = [ { folder = "${home}/Downloads"; } ];
      show-recents = false;
    };
    CustomUserPreferences = {
      NSGlobalDomain = {
        AppleMenuBarVisibleInFullscreen = true;
      };
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
