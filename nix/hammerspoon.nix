{ lib }:
let
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

  fraction =
    name: f:
    "  [\"${name}\"] = { x = ${toString f.x}, y = ${toString f.y}, w = ${toString f.w}, h = ${toString f.h} },";
  fractions = lib.concatStringsSep "\n" (lib.mapAttrsToList fraction tiles);
  aliases = lib.concatStringsSep "\n" (
    lib.mapAttrsToList (alias: target: "  [\"${alias}\"] = \"${target}\",") tileAliases
  );
in
{
  initLua = ''
    hs.window.animationDuration = 0
    hs.dockIcon(false)
    hs.menuIcon(false)
    hs.autoLaunch(false)
    hs.consoleOnTop(false)

    local DARK_TOGGLE = [=[
    ObjC.import("Foundation");
    ObjC.bindFunction("dlopen", ["pointer", ["string", "int"]]);
    $.dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", 1);
    ObjC.bindFunction("SLSGetAppearanceThemeLegacy", ["bool", []]);
    ObjC.bindFunction("SLSSetAppearanceThemeNotifying", ["void", ["bool", "bool"]]);
    $.SLSSetAppearanceThemeNotifying(!$.SLSGetAppearanceThemeLegacy(), true);
    ]=]

    local function toggleDark()
      hs.osascript.javascript(DARK_TOGGLE)
    end

    local function openDefaultBrowser()
      local handler = hs.urlevent.getDefaultHandler("http")
      if handler then
        hs.application.launchOrFocusByBundleID(handler)
      end
    end

    local GAP = ${toString tileGap}

    local FRACTIONS = {
    ${fractions}
    }

    local ALIASES = {
    ${aliases}
    }

    local function resolve(name)
      if name == nil then return nil end
      name = string.lower(name)
      name = ALIASES[name] or name
      if name == "fullscreen" then return "fullscreen" end
      if FRACTIONS[name] then return name end
      return nil
    end

    local function place(win, f)
      local area = win:screen():frame()
      win:setFrame({
        x = area.x + area.w * f.x + GAP,
        y = area.y + area.h * f.y + GAP,
        w = area.w * f.w - GAP * 2,
        h = area.h * f.h - GAP * 2,
      })
    end

    local function tile(name)
      local action = resolve(name)
      if action == nil or not hs.accessibilityState() then
        return
      end
      local win = hs.window.focusedWindow()
      if win == nil then
        return
      end
      if action == "fullscreen" then
        win:toggleFullScreen()
        return
      end
      if win:isFullScreen() then
        return
      end
      place(win, FRACTIONS[action])
    end

    local HYPER = { "cmd", "ctrl", "alt", "shift" }

    local TILE_KEYS = {
      left = "left",
      right = "right",
      up = "top",
      down = "bottom",
      ["return"] = "fullscreen",
      ["1"] = "first-fourth",
      ["2"] = "second-fourth",
      ["3"] = "third-fourth",
      ["4"] = "last-fourth",
      c = "center",
      v = "top-left",
      w = "last-three-fourths",
      m = "maximize",
    }

    for key, action in pairs(TILE_KEYS) do
      hs.hotkey.bind(HYPER, key, function()
        tile(action)
      end)
    end

    hs.hotkey.bind(HYPER, "j", openDefaultBrowser)
    hs.hotkey.bind(HYPER, "`", toggleDark)

    hs.pathwatcher.new(os.getenv("HOME") .. "/.hammerspoon/", function()
      hs.reload()
    end):start()
  '';
}
