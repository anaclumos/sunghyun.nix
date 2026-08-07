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
{
  system.defaults = {
    NSGlobalDomain = {
      # Intentionally minimal. Add only keys verified on the live machine.
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
}
