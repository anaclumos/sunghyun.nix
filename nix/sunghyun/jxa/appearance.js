ObjC.import("Foundation");
ObjC.bindFunction("dlopen", ["pointer", ["string", "int"]]);
$.dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", 1);
ObjC.bindFunction("SLSGetAppearanceThemeLegacy", ["bool", []]);
ObjC.bindFunction("SLSSetAppearanceThemeNotifying", ["void", ["bool", "bool"]]);

// SkyLight talks to the window server directly. The AppleScript route needs
// kTCCServiceAppleEvents for whatever process sends the event, which is a
// second consent prompt and a second privacy row.
function run(argv) {
  var mode = argv.length > 0 ? argv[0] : "toggle";
  var dark = $.SLSGetAppearanceThemeLegacy();
  if (mode === "status") {
    return "dark_mode=" + dark;
  }
  var wanted = mode === "toggle" ? !dark : mode === "dark";
  $.SLSSetAppearanceThemeNotifying(wanted, true);
  var now = $.SLSGetAppearanceThemeLegacy();
  if (now !== wanted) {
    throw new Error("window server kept dark_mode=" + now + " after asking for " + wanted);
  }
  return "dark_mode=" + now;
}
