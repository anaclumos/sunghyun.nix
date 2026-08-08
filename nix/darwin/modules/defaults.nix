{ config, ... }:
let
  home = config.users.users.${config.system.primaryUser}.home;
in
{
  system.defaults = {
    NSGlobalDomain = {
      "com.apple.keyboard.fnState" = false;
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
      "com.apple.finder" = {
        DesktopViewSettings = {
          IconViewSettings = {
            arrangeBy = "grid";
            backgroundColorBlue = 1.0;
            backgroundColorGreen = 1.0;
            backgroundColorRed = 1.0;
            backgroundType = 0;
            gridOffsetX = 0.0;
            gridOffsetY = 0.0;
            gridSpacing = 54.0;
            iconSize = 64.0;
            labelOnBottom = false;
            showIconPreview = true;
            showItemInfo = true;
            textSize = 12.0;
            viewOptionsVersion = 1;
          };
        };
      };
      "com.kakao.KakaoTalkMac" = {
        AppleLanguages = [ "ko" ];
      };
    };
  };
}
