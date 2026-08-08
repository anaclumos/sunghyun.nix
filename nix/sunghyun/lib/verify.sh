cmd_verify() {
  local format=plain
  while [ $# -gt 0 ]; do
    case "$1" in
      --json) format=json ;;
      *) die "usage: sunghyun verify [--json]" ;;
    esac
    shift
  done
  local headless=false
  is_headless && headless=true

  step ok cli "features: open,default-browser,input-source,tile,toggle-dark-mode,hotkeys,fn-state,spotlight,verify,post-switch,kanata,virt"
  check_virtualization
  check_cursor_agent
  check_coding_cli codex codex codex
  check_coding_cli claude claude claude-code
  check_tokenmaxxing
  check_dia
  check_tailscale
  check_default_browser
  check_dock
  check_finder_bars
  check_desktop_icons
  check_locale_units
  check_kakaotalk_language
  check_ime_mapping
  check_apps
  check_tiles
  check_tiling_engine
  check_keyboard_engine
  check_keyboard_grab
  check_fn_state
  check_reserved_hotkeys
  check_fn_tap
  check_kanata_config
  check_hushlogin
  check_spotlight
  check_spotlight_clipboard
  check_terminal_alias
  check_menubar
  check_accessibility
  check_input_monitoring
  check_fonts

  print_report "$format" "$headless"
}

# Informational, never a failure: names the machine class so a run's log shows
# why the App Store surface behaved the way it did.
check_virtualization() {
  step ok virtualization "$(cmd_virt || true)"
}

# OUTCOMES.md row p: the Cursor Agent CLI is present. macOS installs it through
# the official cursor-cli Homebrew cask; Linux gets the nixpkgs package.
check_cursor_agent() {
  local found
  if found="$(first_existing /opt/homebrew/bin/cursor-agent /usr/local/bin/cursor-agent "$HOME/.local/bin/cursor-agent")" ||
    found="$(command -v cursor-agent 2>/dev/null)"; then
    step ok cursor_agent "cursor-agent present ($found)"
  elif is_headless; then
    step skipped cursor_agent "cursor-agent not on PATH (headless; the portable layer installs it on the next switch)"
  elif [ ! -x /opt/homebrew/bin/brew ]; then
    step skipped cursor_agent "Homebrew absent, so the cursor-cli cask could not install yet; converges next switch"
  else
    step failed cursor_agent "cursor-agent missing; the cursor-cli cask (macOS) / nixpkgs cursor-cli (Linux) should have installed it"
  fi
}

# OUTCOMES.md row v: Codex and Claude Code CLIs present. The cask token and the
# binary name differ for Claude Code.
check_coding_cli() {
  local id="$1" binary="$2" package="$3" found
  if found="$(first_existing "/opt/homebrew/bin/$binary" "/usr/local/bin/$binary")" ||
    found="$(command -v "$binary" 2>/dev/null)"; then
    step ok "$id" "$binary present ($found)"
  elif is_headless; then
    step skipped "$id" "$binary not on PATH (headless; the portable layer installs it on the next switch)"
  elif [ ! -x /opt/homebrew/bin/brew ]; then
    step skipped "$id" "Homebrew absent, so the $package cask could not install yet; converges next switch"
  else
    step failed "$id" "$binary missing; the $package cask (macOS) / nixpkgs $package (Linux) should have installed it"
  fi
}

# OUTCOMES.md row aj: tokenmaxxing comes from the flake input, never Homebrew,
# so the Nix profile dirs plus PATH are the only probes.
check_tokenmaxxing() {
  local found
  if found="$(first_existing /run/current-system/sw/bin/tokenmaxxing "$HOME/.nix-profile/bin/tokenmaxxing")" ||
    found="$(command -v tokenmaxxing 2>/dev/null)"; then
    step ok tokenmaxxing "tokenmaxxing present ($found)"
  else
    step failed tokenmaxxing "tokenmaxxing missing; the github:anaclumos/tokenmaxxing flake input should have installed it"
  fi
}

# OUTCOMES.md row aa: Dia present, installed by the thebrowsercompany-dia cask.
check_dia() {
  if [ -d /Applications/Dia.app ]; then
    step ok dia "/Applications/Dia.app present"
  elif is_headless; then
    step skipped dia "Dia absent (headless; the cask installs it on a GUI Mac)"
  else
    step failed dia "Dia missing; the thebrowsercompany-dia cask should have installed it"
  fi
}

# OUTCOMES.md row ag: Tailscale present so tailnet MagicDNS names resolve after
# the owner signs in. The standalone app bundles daemon, GUI and CLI.
check_tailscale() {
  if [ -d /Applications/Tailscale.app ]; then
    step ok tailscale "/Applications/Tailscale.app present"
  elif is_headless; then
    step skipped tailscale "Tailscale absent (headless; the cask installs it on a GUI Mac)"
  else
    step failed tailscale "Tailscale missing; the tailscale-app cask should have installed it"
  fi
}

# OUTCOMES.md row ab: Dia is the system default browser, so Hyper+J opens it.
check_default_browser() {
  local current
  current="$(default_browser_current)"
  if [ "$current" = "$DIA_BUNDLE_ID" ]; then
    step ok default_browser "http handler is Dia ($current)"
  elif [ -z "$current" ] || [ "$current" = unknown ]; then
    step skipped default_browser "LaunchServices reports no http handler"
  elif is_headless; then
    step skipped default_browser "http handler is $current (headless; the confirmation panel needs a GUI session)"
  else
    step failed default_browser "http handler is $current; macOS's confirmation panel decides this one and post-switch raises it"
  fi
}

# OUTCOMES.md row ac: the Dock holds Finder, the Downloads folder tile and the
# Trash, nothing else. Finder and the Trash are not preferences.
check_dock() {
  if is_headless; then
    step skipped dock "Dock state needs a GUI macOS session"
    return
  fi
  local apps others other_tiles downloads=false recents=false
  apps="$(defaults_read com.apple.dock persistent-apps | grep -c tile-data || true)"
  others="$(defaults_read com.apple.dock persistent-others || true)"
  other_tiles="$(printf '%s' "$others" | grep -c tile-data || true)"
  case "$others" in
    */Downloads*) downloads=true ;;
  esac
  [ "$(defaults_read com.apple.dock show-recents || true)" = 0 ] && recents=true
  if [ "$apps" = 0 ] && [ "$other_tiles" = 1 ] && [ "$downloads" = true ] && [ "$recents" = true ]; then
    step ok dock "Dock holds only the Downloads folder beside Finder and the Trash"
  else
    step failed dock "Dock pinned: $apps apps, $other_tiles others, Downloads=$downloads, show-recents off=$recents"
  fi
}

# OUTCOMES.md row ah: Finder windows show the path bar and the status bar.
check_finder_bars() {
  if is_headless; then
    step skipped finder_bars "Finder windows need a GUI macOS session"
    return
  fi
  local pathbar statusbar
  pathbar="$(defaults_read com.apple.finder ShowPathbar || echo 0)"
  statusbar="$(defaults_read com.apple.finder ShowStatusBar || echo 0)"
  if [ "$pathbar" = 1 ] && [ "$statusbar" = 1 ]; then
    step ok finder_bars "path bar and status bar shown"
  else
    step failed finder_bars "ShowPathbar=$pathbar, ShowStatusBar=$statusbar"
  fi
}

# OUTCOMES.md rows ad and ai: hard disks on the Desktop, item info under each
# icon, labels to the right, icons snapping to the grid.
check_desktop_icons() {
  if is_headless; then
    step skipped desktop_icons "Desktop icons need a GUI macOS session"
    return
  fi
  local disks item_info label_bottom arrange_by
  disks="$(defaults_read com.apple.finder ShowHardDrivesOnDesktop || echo 0)"
  item_info="$(defaults_extract com.apple.finder DesktopViewSettings.IconViewSettings.showItemInfo || echo unset)"
  label_bottom="$(defaults_extract com.apple.finder DesktopViewSettings.IconViewSettings.labelOnBottom || echo unset)"
  arrange_by="$(defaults_extract com.apple.finder DesktopViewSettings.IconViewSettings.arrangeBy || echo unset)"
  if [ "$disks" = 1 ] && [ "$item_info" = true ] && [ "$label_bottom" = false ] && [ "$arrange_by" = grid ]; then
    step ok desktop_icons "hard disks shown, item info on, labels on the right, snap to grid"
  else
    step failed desktop_icons "hard disks=$disks, showItemInfo=$item_info, labelOnBottom=$label_bottom, arrangeBy=$arrange_by"
  fi
}

# OUTCOMES.md row ae: Celsius and metric. macOS reads three separate keys and
# disagrees with itself when only some are set.
check_locale_units() {
  if is_headless; then
    step skipped locale_units "locale units need a GUI macOS session"
    return
  fi
  local temp measure metric
  temp="$(defaults_read -g AppleTemperatureUnit || echo unset)"
  measure="$(defaults_read -g AppleMeasurementUnits || echo unset)"
  metric="$(defaults_read -g AppleMetricUnits || echo unset)"
  if [ "$temp" = Celsius ] && [ "$measure" = Centimeters ] && [ "$metric" = 1 ]; then
    step ok locale_units "Celsius, Centimeters, metric"
  else
    step failed locale_units "AppleTemperatureUnit=$temp, AppleMeasurementUnits=$measure, AppleMetricUnits=$metric"
  fi
}

# OUTCOMES.md row af: KakaoTalk runs in Korean whatever the system language is.
# It is sandboxed, so an unreadable container is a skip, not a failure.
check_kakaotalk_language() {
  if is_headless || [ ! -d /Applications/KakaoTalk.app ]; then
    step skipped kakaotalk_language "KakaoTalk not installed here (mas converges it later)"
    return
  fi
  local languages
  if ! languages="$(defaults_read com.kakao.KakaoTalkMac AppleLanguages)"; then
    step skipped kakaotalk_language "KakaoTalk's sandbox container is not readable from here; Language & Region owns the value"
    return
  fi
  case "$languages" in
    *ko*) step ok kakaotalk_language "KakaoTalk AppleLanguages = ko" ;;
    *) step failed kakaotalk_language "KakaoTalk AppleLanguages = $(printf '%s' "$languages" | tr '\n' ' ')" ;;
  esac
}

check_ime_mapping() {
  local abc korean
  abc="$(resolve_ime ABC || true)"
  korean="$(resolve_ime 2SetKorean || true)"
  if [ -n "$abc" ] && [ -n "$korean" ]; then
    step ok ime_map "ABC=$abc korean=$korean"
  else
    step failed ime_map "IME id mapping incomplete"
  fi
}

check_apps() {
  local key missing="" required="calendar ghostty iina linear mail preview slack"
  for key in $required; do
    resolve_app "$key" >/dev/null || missing="$missing $key"
  done
  if [ -z "$missing" ]; then
    step ok apps "$(printf '%s' "$required" | wc -w | tr -d ' ') app keys resolvable"
  else
    step failed apps "missing keys:$missing"
  fi
}

check_tiles() {
  local name missing="" all="$TILE_ACTIONS"
  for name in $all; do
    resolve_tile "$name" >/dev/null || missing="$missing $name"
  done
  if [ -z "$missing" ]; then
    step ok tiles "$(printf '%s' "$all" | wc -w | tr -d ' ') tile actions mapped"
  else
    step failed tiles "tile action parse incomplete:$missing"
  fi
}

# Outcome: something is running that can place the focused window. Hammerspoon
# owns the Accessibility API call, so its message port has to answer.
check_tiling_engine() {
  if is_headless; then
    step skipped tiling_engine "window placement needs a GUI macOS session"
    return
  fi
  if [ ! -x "$HS_CLI" ]; then
    step failed tiling_engine "Hammerspoon missing; the hammerspoon cask should have installed it"
    return
  fi
  local answer
  if answer="$("$HS_CLI" -c "tileActions()" 2>&1)" && [ -n "$answer" ]; then
    step ok tiling_engine "Hammerspoon reachable with $(printf '%s' "$answer" | wc -w | tr -d ' ') tile actions"
  else
    step failed tiling_engine "Hammerspoon is installed but its message port did not answer ($answer)"
  fi
}

# Outcome check (OUTCOMES.md a-e): a tap-hold keyboard engine is configured with
# the sunghyun binding set. Asserts outcome tokens, not engine internals, so the
# engine can be swapped without touching this check.
check_keyboard_engine() {
  local karabiner="$HOME/.config/karabiner/karabiner.json" text missing=""
  if ! text="$(cat "$karabiner" 2>/dev/null)"; then
    step skipped keyboard_engine "no karabiner.json yet (darwin-rebuild switch materializes it); kanata remains the opt-in alternative"
    return
  fi
  local pair name token
  # ⌘⇧V sends virtual ⌘Space then ⌘4, so spacebar is the token for that rule.
  for pair in \
    "caps tap = maximize|tile maximize" \
    "hyper tiling|tile left" \
    "hyper+w right three quarters|tile last-three-fourths" \
    "hyper browser|open-default-browser" \
    "hyper+i iina|open iina" \
    "hyper+n slack|open slack" \
    "hyper+p preview|open preview" \
    "hyper+r linear|open linear" \
    "hyper+grave dark mode|toggle-dark-mode" \
    "cmd tap = IME|input-source ABC" \
    "cmd-shift-v clipboard|spacebar"; do
    name="${pair%%|*}"
    token="${pair#*|}"
    case "$text" in
      *"$token"*) ;;
      *) missing="$missing, $name" ;;
    esac
  done
  if [ -z "$missing" ]; then
    step ok keyboard_engine "karabiner.json covers outcomes a-e ($karabiner)"
  else
    step failed keyboard_engine "karabiner.json missing outcomes: ${missing#, }"
  fi
}

# OUTCOMES.md row l: a Core-Service that missed a config relink is the wedge that
# eats every key, so read-only detection belongs here; post-switch owns the heal.
check_keyboard_grab() {
  if is_headless; then
    step skipped keyboard_grab "keyboard grab health needs a GUI macOS session"
    return
  fi
  local relinked reloaded
  if ! relinked="$(karabiner_config_mtime)" || [ ! -r "$KARABINER_LOG" ] ||
    ! reloaded="$(karabiner_last_reload_epoch)"; then
    step skipped keyboard_grab "no karabiner.json relink and reload pair to compare yet"
  elif [ "$reloaded" -ge "$relinked" ]; then
    step ok keyboard_grab "Core-Service reloaded the config after the last relink (grab cycle healthy)"
  else
    step failed keyboard_grab "Core-Service has not logged a reload since the last karabiner.json relink; it may be holding the grab while deaf (run \`sunghyun karabiner health\`)"
  fi
}

# OUTCOMES.md row o: the media top row is only real if IOHIDSystem agrees with
# the declared preference, not just the plist.
check_fn_state() {
  local mode
  if ! mode="$(fn_state_mode)"; then
    step skipped fn_state "IOHIDSystem does not report HIDFKeyMode here"
  elif [ "$mode" = 0 ]; then
    step ok fn_state "top row fires media bare (HIDFKeyMode=0)"
  else
    step failed fn_state "IOHIDSystem enforces HIDFKeyMode=$mode; the declared media top row has not converged"
  fi
}

# OUTCOMES.md row w: ⌘⇧Space belongs to 1Password, so no macOS symbolic hot key
# may still be sitting on it.
check_reserved_hotkeys() {
  if is_headless; then
    step skipped reserved_hotkeys "reserved chords skipped in headless (no window server)"
    return
  fi
  local still
  still="$(jxa hotkeys scan | awk '$5 == "true" { print "symbolic hot key " $1 " still claims the chord" }')"
  if [ -z "$still" ]; then
    step ok reserved_hotkeys "⌘⇧Space reaches 1Password only (no system shortcut claims it)"
  else
    step failed reserved_hotkeys "$(printf '%s' "$still" | tr '\n' ';')"
  fi
}

# OUTCOMES.md row u: a bare fn tap opens the Emoji & Symbols picker.
# AppleFnUsageType governs the bare tap only; the fn+F-row inversion rides
# HIDFKeyMode (check_fn_state), so the two checks cannot collide.
check_fn_tap() {
  if is_headless; then
    step skipped fn_tap "fn tap check skipped (headless; no keyboard UI)"
    return
  fi
  local value
  if ! value="$(defaults_read com.apple.HIToolbox AppleFnUsageType)"; then
    step ok fn_tap "AppleFnUsageType unset; macOS defaults the bare fn tap to Emoji & Symbols"
  elif [ "$value" = 2 ]; then
    step ok fn_tap "bare fn tap opens Emoji & Symbols (AppleFnUsageType=2)"
  else
    step failed fn_tap "AppleFnUsageType=$value; expected 2 (Show Emoji & Symbols); darwin-rebuild switch declares it"
  fi
}

check_kanata_config() {
  local kbd="$CONFIG_DIR/kanata.kbd" raw
  if ! raw="$(cat "$kbd" 2>/dev/null)"; then
    if is_headless; then
      step skipped kanata_kbd "kanata.kbd not found under ~/.config/sunghyun (ok to provision later)"
    else
      step failed kanata_kbd "kanata.kbd not found; darwin-rebuild switch materializes it"
    fi
    return
  fi
  case "$raw" in
    *"clipboard show"*)
      step failed kanata_kbd "$kbd still binds the clipboard picker; ⌘⇧V (native macro) owns clipboard"
      return
      ;;
  esac
  local token
  for token in "@lcmd" "@rcmd" lmet rmet "tile maximize" "M-spc"; do
    case "$raw" in
      *"$token"*) ;;
      *)
        step failed kanata_kbd "$kbd missing $token"
        return
        ;;
    esac
  done
  case "$raw" in
    *"spotlight clipboard"*)
      step failed kanata_kbd "$kbd must bind ⌘⇧V as a native macro (M-spc then M-4), not a CLI hop"
      return
      ;;
  esac
  step ok kanata_kbd "found $kbd (⌘ tap=IME hold=mod; Caps maximize; ⌘⇧V Spotlight clipboard)"
}

check_hushlogin() {
  if [ -f "$HOME/.hushlogin" ]; then
    step ok hushlogin "$HOME/.hushlogin present"
  elif is_headless; then
    step skipped hushlogin "$HOME/.hushlogin missing (ok in headless; bootstrap creates it)"
  else
    step failed hushlogin "$HOME/.hushlogin missing; darwin-rebuild/home-manager switch materializes it"
  fi
}

check_spotlight() {
  if is_headless; then
    step skipped spotlight "Spotlight check skipped (headless)"
  elif spotlight_command_space_enabled; then
    step ok spotlight "⌘Space Show Spotlight search enabled"
  else
    step failed spotlight "Spotlight ⌘Space disabled; run \`sunghyun spotlight restore\` or enable in System Settings"
  fi
}

check_spotlight_clipboard() {
  if is_headless; then
    step skipped spotlight_clipboard "Spotlight pasteboard check skipped (headless)"
  elif pasteboard_history_enabled; then
    step ok spotlight_clipboard "Clipboard History on; ⌘⇧V sends ⌘Space then ⌘4 via Karabiner (Apple has no native global hotkey)"
  else
    step failed spotlight_clipboard "PasteboardHistoryEnabled off; run \`sunghyun spotlight restore\` or enable Clipboard History in System Settings → Spotlight"
  fi
}

check_terminal_alias() {
  if terminal_alias_current; then
    step ok terminal_alias "$HOME/Applications/terminal.app opens Ghostty (Spotlight query: terminal)"
  elif is_headless; then
    step skipped terminal_alias "terminal.app alias missing (ok headless; bootstrap installs on GUI Mac)"
  else
    step failed terminal_alias "$HOME/Applications/terminal.app missing; run \`sunghyun spotlight restore\`"
  fi
}

check_menubar() {
  if is_headless; then
    step skipped menubar "menu bar check skipped (headless)"
    return
  fi
  local tm=false cursor=false
  time_machine_hidden && tm=true
  cursor_tray_hidden && cursor=true
  if [ "$tm" = true ] && [ "$cursor" = true ]; then
    step ok menubar "Time Machine + Cursor hidden from menu bar"
  else
    step failed menubar "menu bar extras still visible (Time Machine hidden=$tm, Cursor tray hidden=$cursor); run \`sunghyun post-switch\` (menubar step)"
  fi
}

# Check-only: never opens Settings or polls (post-switch owns the gate). The
# subject is Hammerspoon, which is the process that calls the Accessibility API.
check_accessibility() {
  if is_headless; then
    step skipped accessibility "Accessibility skipped (headless)"
    return
  fi
  if [ ! -x "$HS_CLI" ]; then
    step skipped accessibility "Hammerspoon not installed yet; the cask installs it on this switch"
    return
  fi
  local answer
  if ! answer="$("$HS_CLI" -c "accessibilityGranted()" 2>&1)"; then
    step failed accessibility "Hammerspoon message port did not answer ($answer)"
  elif [ "$answer" = true ]; then
    step ok accessibility "Accessibility granted to Hammerspoon (window placement works)"
  else
    step failed accessibility "Accessibility not granted to Hammerspoon; post-switch opens the pane and waits for the toggle"
  fi
}

check_input_monitoring() {
  if is_headless; then
    step skipped input_monitoring "skipped (headless; Kanata N/A)"
  elif ! have kanata; then
    step skipped input_monitoring "kanata not on PATH (opt-in engine)"
  elif kanata_running; then
    # The only real observable without Full Disk Access: a running kanata holds
    # the IOHID grab, which is impossible without the grant.
    step ok input_monitoring "kanata running and holding the input grab (grant proven)"
  else
    step skipped input_monitoring "advisory: kanata installed but not running; Input Monitoring not probeable without FDA (safe-enable proves it)"
  fi
}

# OUTCOMES.md row v: Sunghyun Sans is visible in the OS font path. macOS
# materializes nix-darwin fonts.packages under /Library/Fonts/Nix Fonts; Linux
# exposes Home Manager fonts through the profile's share/fonts.
check_fonts() {
  local found
  found="$(find "/Library/Fonts/Nix Fonts" "$HOME/.nix-profile/share/fonts" \
    -name 'SunghyunSans*' -print -quit 2>/dev/null || true)"
  if [ -n "$found" ]; then
    step ok fonts "Sunghyun Sans installed ($found)"
  elif is_headless; then
    step skipped fonts "Sunghyun Sans not found yet (headless; the next switch installs it)"
  else
    step failed fonts "Sunghyun Sans missing from /Library/Fonts/Nix Fonts and ~/.nix-profile/share/fonts; darwin-rebuild/home-manager switch installs it"
  fi
}
