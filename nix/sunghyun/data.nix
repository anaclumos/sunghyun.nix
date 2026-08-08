{
  ime = {
    # "ABC" is the outcome name; this Mac enables the U.S. layout, and
    # TISSelectInputSource returns paramErr for an installed-but-not-enabled
    # source, so the enabled id is the one that has to be named here.
    abc = "com.apple.keylayout.US";
    korean = "com.apple.inputmethod.Korean.2SetKorean";
  };

  apps = {
    calendar = "com.apple.iCal";
    ghostty = "com.mitchellh.ghostty";
    iina = "com.colliderli.iina";
    kakaotalk = "com.kakao.KakaoTalkMac";
    linear = "com.linear";
    mail = "com.apple.mail";
    music = "com.apple.Music";
    preview = "com.apple.Preview";
    slack = "com.tinyspeck.slackmacgap";
    tableplus = "com.tinyapp.TablePlus";
  };

  appAliases = {
    terminal = "ghostty";
    planetscale = "tableplus";
  };

  # Fractions of a display's usable area (menu bar and Dock excluded).
  tiles = {
    left = {
      x = 0.0;
      y = 0.0;
      w = 0.5;
      h = 1.0;
    };
    right = {
      x = 0.5;
      y = 0.0;
      w = 0.5;
      h = 1.0;
    };
    top = {
      x = 0.0;
      y = 0.0;
      w = 1.0;
      h = 0.5;
    };
    bottom = {
      x = 0.0;
      y = 0.5;
      w = 1.0;
      h = 0.5;
    };
    center = {
      x = 0.125;
      y = 0.125;
      w = 0.75;
      h = 0.75;
    };
    "top-left" = {
      x = 0.0;
      y = 0.0;
      w = 0.5;
      h = 0.5;
    };
    "first-fourth" = {
      x = 0.0;
      y = 0.0;
      w = 0.25;
      h = 1.0;
    };
    "second-fourth" = {
      x = 0.25;
      y = 0.0;
      w = 0.25;
      h = 1.0;
    };
    "third-fourth" = {
      x = 0.5;
      y = 0.0;
      w = 0.25;
      h = 1.0;
    };
    "last-fourth" = {
      x = 0.75;
      y = 0.0;
      w = 0.25;
      h = 1.0;
    };
    "last-three-fourths" = {
      x = 0.25;
      y = 0.0;
      w = 0.75;
      h = 1.0;
    };
    maximize = {
      x = 0.0;
      y = 0.0;
      w = 1.0;
      h = 1.0;
    };
    "right-third" = {
      x = 0.6666666666666666;
      y = 0.0;
      w = 0.3333333333333333;
      h = 1.0;
    };
  };

  tileAliases = {
    "left-half" = "left";
    "right-half" = "right";
    "top-half" = "top";
    "bottom-half" = "bottom";
    "top-left-quarter" = "top-left";
    "1" = "first-fourth";
    "2" = "second-fourth";
    "3" = "third-fourth";
    "4" = "last-fourth";
    max = "maximize";
    "last-third" = "right-third";
    "toggle-fullscreen" = "fullscreen";
  };

  tileGap = 0;

  # Chords that belong to an app, not to macOS. Matched by chord rather than by
  # symbolic hot key identifier: Apple renumbers these between releases.
  reservedChords = [
    {
      reservedFor = "1Password Quick Access";
      virtualKey = 49;
      # command | shift
      modifiers = 1179648;
    }
  ];

  defaultBrowserBundleId = "company.thebrowser.dia";

  terminalAlias = {
    bundleId = "com.anaclumos.terminal-ghostty";
    target = "com.mitchellh.ghostty";
  };

  kanata = {
    label = "com.anaclumos.kanata";
    minVersion = "1.12.0";
    driverPkgUrl = "https://github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice/releases/download/v6.2.0/Karabiner-DriverKit-VirtualHIDDevice-6.2.0.pkg";
  };
}
