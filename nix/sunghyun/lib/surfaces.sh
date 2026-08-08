CONTROL_CENTER_DONT_SHOW_IN_MENU_BAR=2
CURSOR_APPLICATION_USER_KEY="src.vs.platform.reactivestorage.browser.reactiveStorageServiceImpl.persistentStorage.applicationUser"

cmd_spotlight() {
  case "${1:-status}" in
    status)
      if is_headless; then
        skip "Spotlight check skipped (headless)"
      fi
      echo "spotlight_command_space_enabled=$(spotlight_command_space_enabled && echo true || echo false)"
      ;;
    restore)
      if is_headless; then
        skip "Spotlight restore skipped (headless / no GUI session)"
      fi
      spotlight_restore
      ;;
    install-terminal-alias) terminal_alias_install ;;
    clipboard)
      # macOS 26+ WindowServer drops synthesized keystrokes before the global
      # hotkey matcher, and Tahoe has no ⌘⇧V symbolic hot key to enable, so the
      # karabiner virtual-HID rule is the only working path.
      die "cannot open Clipboard Search from a CLI: press ⌘⇧V (karabiner virtual-HID rule) or ⌘Space then ⌘4 on the keyboard"
      ;;
    *) die "usage: sunghyun spotlight [status|restore|install-terminal-alias|clipboard]" ;;
  esac
}

# ⌘Space is symbolic hot key 64. `-dict-add` patches only that identifier, so
# every other system shortcut in AppleSymbolicHotKeys survives.
spotlight_command_space_enabled() {
  local value
  value="$(defaults_read com.apple.symbolichotkeys "AppleSymbolicHotKeys.$SPOTLIGHT_HOTKEY_ID.enabled" 2>/dev/null)" || return 0
  [ "$value" = 1 ]
}

spotlight_restore_command_space() {
  /usr/bin/defaults write com.apple.symbolichotkeys AppleSymbolicHotKeys -dict-add "$SPOTLIGHT_HOTKEY_ID" \
    "<dict><key>enabled</key><true/><key>value</key><dict><key>type</key><string>standard</string><key>parameters</key><array><integer>32</integer><integer>49</integer><integer>1048576</integer></array></dict></dict>"
  [ -x "$ACTIVATE_SETTINGS" ] && "$ACTIVATE_SETTINGS" -u
  return 0
}

pasteboard_history_enabled() {
  local value
  value="$(defaults_read com.apple.Spotlight PasteboardHistoryEnabled 2>/dev/null)" || return 0
  [ "$value" = 1 ]
}

# Converges all three Spotlight outcomes. An early return on "⌘Space is already
# enabled" is wrong: that is the factory default, so a fresh Mac would exit
# before ever installing ~/Applications/terminal.app.
spotlight_restore() {
  local done_parts=""
  if spotlight_command_space_enabled; then
    done_parts="⌘Space"
  else
    spotlight_restore_command_space
    done_parts="⌘Space restored"
  fi
  /usr/bin/defaults write com.apple.Spotlight PasteboardHistoryEnabled -bool true
  done_parts="$done_parts, Clipboard History"
  terminal_alias_install >/dev/null
  done_parts="$done_parts, terminal→Ghostty alias"
  echo "$done_parts"
}

terminal_alias_path() {
  printf '%s' "$HOME/Applications/terminal.app"
}

terminal_alias_current() {
  local app plist exe
  app="$(terminal_alias_path)"
  plist="$app/Contents/Info.plist"
  exe="$app/Contents/MacOS/terminal"
  [ -f "$plist" ] && [ -f "$exe" ] || return 1
  grep -q "$TERMINAL_ALIAS_BUNDLE_ID" "$plist" &&
    grep -q "<string>terminal</string>" "$plist" &&
    grep -q "$TERMINAL_ALIAS_TARGET" "$exe"
}

# Spotlight Quick Keys are for actions, not app aliases. A thin app named
# `terminal` makes typing "terminal" match Ghostty without deleting Apple's
# Terminal.app.
terminal_alias_install() {
  if is_headless; then
    skip "terminal→Ghostty alias skipped (headless)"
  fi
  local app
  app="$(terminal_alias_path)"
  if terminal_alias_current; then
    echo "$app already opens Ghostty"
    return 0
  fi
  mkdir -p "$app/Contents/MacOS"
  cat >"$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>terminal</string>
	<key>CFBundleExecutable</key>
	<string>terminal</string>
	<key>CFBundleIdentifier</key>
	<string>$TERMINAL_ALIAS_BUNDLE_ID</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>terminal</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>1.0</string>
	<key>CFBundleVersion</key>
	<string>1</string>
	<key>LSMinimumSystemVersion</key>
	<string>14.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSAppleScriptEnabled</key>
	<false/>
</dict>
</plist>
PLIST
  cat >"$app/Contents/MacOS/terminal" <<SCRIPT
#!/bin/bash
exec /usr/bin/open -b $TERMINAL_ALIAS_TARGET "\$@"
SCRIPT
  printf 'APPL????' >"$app/Contents/PkgInfo"
  chmod 755 "$app/Contents/MacOS/terminal"
  /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app" || true
  echo "$app installed"
}

time_machine_hidden() {
  local visible_cc visible cc
  visible_cc="$(defaults_read com.apple.systemuiserver "NSStatusItem VisibleCC com.apple.menuextra.TimeMachine" || true)"
  visible="$(defaults_read com.apple.systemuiserver "NSStatusItem Visible com.apple.menuextra.TimeMachine" || true)"
  cc="$(/usr/bin/defaults -currentHost read com.apple.controlcenter TimeMachine 2>/dev/null || printf unreadable)"
  case "$visible_cc$visible" in
    *0*) ;;
    *) return 1 ;;
  esac
  [ "$cc" = "$CONTROL_CENTER_DONT_SHOW_IN_MENU_BAR" ] || [ "$cc" = unreadable ]
}

hide_time_machine() {
  /usr/bin/defaults -currentHost write com.apple.controlcenter TimeMachine -int "$CONTROL_CENTER_DONT_SHOW_IN_MENU_BAR"
  /usr/bin/defaults write com.apple.systemuiserver "NSStatusItem VisibleCC com.apple.menuextra.TimeMachine" -bool false
  /usr/bin/defaults write com.apple.systemuiserver "NSStatusItem Visible com.apple.menuextra.TimeMachine" -bool false
  /usr/bin/defaults write com.apple.systemuiserver menuExtras -array
  /usr/bin/killall SystemUIServer >/dev/null 2>&1 || true
  /usr/bin/killall ControlCenter >/dev/null 2>&1 || true
}

cursor_state_db() {
  printf '%s' "$HOME/Library/Application Support/Cursor/User/globalStorage/state.vscdb"
}

cursor_tray_hidden() {
  local db
  db="$(cursor_state_db)"
  [ -f "$db" ] || return 0
  local raw
  raw="$(/usr/bin/sqlite3 "$db" "SELECT value FROM ItemTable WHERE key='$CURSOR_APPLICATION_USER_KEY';" 2>/dev/null || true)"
  case "$raw" in
    *'"systemTrayEnabled":false'*) return 0 ;;
    *) return 1 ;;
  esac
}

hide_cursor_tray() {
  local db raw updated
  db="$(cursor_state_db)"
  [ -f "$db" ] || {
    echo "skipped: Cursor state.vscdb missing (install/launch Cursor first)"
    return 0
  }
  raw="$(/usr/bin/sqlite3 "$db" "SELECT value FROM ItemTable WHERE key='$CURSOR_APPLICATION_USER_KEY';" 2>/dev/null || true)"
  [ -n "$raw" ] || {
    echo "skipped: Cursor applicationUser storage row missing"
    return 0
  }
  case "$raw" in
    *'"systemTrayEnabled":'*)
      updated="$(printf '%s' "$raw" | sed 's/"systemTrayEnabled":true/"systemTrayEnabled":false/')"
      ;;
    *) updated="$(printf '%s' "$raw" | sed 's/^{/{"systemTrayEnabled":false,/')" ;;
  esac
  updated="$(printf '%s' "$updated" | sed "s/'/''/g")"
  /usr/bin/sqlite3 "$db" "UPDATE ItemTable SET value='$updated' WHERE key='$CURSOR_APPLICATION_USER_KEY';"
  echo "ok"
}
