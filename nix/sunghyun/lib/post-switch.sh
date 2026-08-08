# Residual steps after `darwin-rebuild switch`. Nix owns packages, daemons and
# files; what is left is macOS's own one-time surfaces (dext approval,
# Accessibility, keyboard-engine first launch, the default-browser panel) plus
# the two live-state restores that no preference can express. Each one opens the
# exact pane or lets the OS prompt, polls, and degrades to a skip on timeout.
DRY_RUN=false

cmd_post_switch() {
  local format=plain
  while [ $# -gt 0 ]; do
    case "$1" in
      --json) format=json ;;
      --dry-run) DRY_RUN=true ;;
      *) die "usage: sunghyun post-switch [--dry-run] [--json]" ;;
    esac
    shift
  done
  local headless=false
  is_headless && headless=true

  if [ "$headless" = true ]; then
    step ok post_switch "headless post-switch (GUI gates skip)"
  else
    step ok post_switch "interactive post-switch after darwin-rebuild"
  fi

  gate_karabiner_health
  gate_karabiner_driverkit
  gate_keyboard_engine
  gate_accessibility
  gate_default_browser
  gate_spotlight
  gate_menubar

  print_report "$format" "$headless"
}

KARABINER_LOG=/var/log/karabiner/core_service.log
KARABINER_AGENT=org.pqrs.service.agent.Karabiner-Core-Service-rev2

# Karabiner 16.1.0's root Core-Service can go deaf while still holding the
# exclusive keyboard grab: three rapid karabiner.json relinks on 2026-08-08 left
# a daemon that logged no reload and ate every key, on-screen keyboard included.
# Restarting the user-domain agent is the no-sudo way back to a full re-grab, so
# a switch that relinked the config without a matching reload triggers it.
cmd_karabiner() {
  case "${1:-health}" in
    health)
      local out
      out="$(karabiner_health)"
      case "$out" in
        failed:*) die "${out#failed: }" ;;
        skipped:*) skip "${out#skipped: }" ;;
        *) printf '%s\n' "${out#ok: }" ;;
      esac
      ;;
    *) die "usage: sunghyun karabiner health" ;;
  esac
}

gate_karabiner_health() {
  if is_headless; then
    step skipped karabiner_health "keyboard engine health needs a GUI macOS session"
    return
  fi
  if [ "$DRY_RUN" = true ]; then
    step skipped karabiner_health "would compare the last config reload against the config relink"
    return
  fi
  step_from_outcome karabiner_health "$(karabiner_health)"
}

karabiner_config_mtime() {
  /usr/bin/stat -f %m "$HOME/.config/karabiner/karabiner.json" 2>/dev/null
}

karabiner_last_reload_epoch() {
  local line stamp
  line="$(grep "core_configuration is updated" "$KARABINER_LOG" 2>/dev/null | tail -1)"
  [ -n "$line" ] || return 1
  stamp="${line%%]*}"
  stamp="${stamp#[}"
  stamp="${stamp%.*}"
  /bin/date -j -f "%Y-%m-%d %H:%M:%S" "$stamp" +%s 2>/dev/null
}

karabiner_grab_count() {
  grep -c "hid queue value monitor is started (grabbed)" "$KARABINER_LOG" 2>/dev/null || echo 0
}

karabiner_health() {
  local relinked reloaded
  if ! relinked="$(karabiner_config_mtime)"; then
    echo "skipped: no karabiner.json yet (darwin-rebuild switch materializes it)"
    return 0
  fi
  if [ ! -r "$KARABINER_LOG" ]; then
    echo "skipped: $KARABINER_LOG not readable; cannot tell a missed reload from a healthy one"
    return 0
  fi
  if ! reloaded="$(karabiner_last_reload_epoch)"; then
    echo "skipped: no config reload logged yet; Karabiner-Elements has not started"
    return 0
  fi
  if [ "$reloaded" -ge "$relinked" ]; then
    echo "ok: Core-Service reloaded the config after the relink (grab cycle healthy)"
    return 0
  fi
  local before uid
  before="$(karabiner_grab_count)"
  uid="$(id -u)"
  echo >&2 "keyboard engine: Core-Service missed the config relink; restarting its user agent"
  /bin/launchctl kickstart -k "gui/$uid/$KARABINER_AGENT" >/dev/null 2>&1 || true
  local waited=0
  while [ "$waited" -lt 30 ]; do
    sleep 2
    waited=$((waited + 2))
    if [ "$(karabiner_grab_count)" -gt "$before" ]; then
      echo "ok: Core-Service had gone deaf holding the grab; the agent restart re-grabbed every keyboard"
      return 0
    fi
  done
  echo "failed: Core-Service missed the relink and did not re-grab within 30s; run \`/bin/launchctl kickstart -k gui/$uid/$KARABINER_AGENT\`"
}

gate_karabiner_driverkit() {
  if is_headless; then
    step skipped karabiner_driverkit "Karabiner-DriverKit skipped (headless)"
    return
  fi
  local installed=false
  if [ -f "/Applications/.Karabiner-VirtualHIDDevice-Manager.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Manager" ] ||
    [ -d "/Library/Application Support/org.pqrs/Karabiner-DriverKit-VirtualHIDDevice" ] ||
    [ -d /Applications/Karabiner-Elements.app ]; then
    installed=true
  fi
  if [ "$installed" = false ]; then
    if [ "$DRY_RUN" = true ]; then
      step skipped karabiner_driverkit "would install Karabiner-DriverKit-VirtualHIDDevice v6.2.0 pkg"
      return
    fi
    local pkg="$CONFIG_DIR/Karabiner-DriverKit-VirtualHIDDevice-6.2.0.pkg"
    mkdir -p "$CONFIG_DIR"
    echo >&2 "Downloading Karabiner-DriverKit v6.2.0"
    if ! /usr/bin/curl -fsSL -o "$pkg" "$KANATA_DRIVER_URL"; then
      step failed karabiner_driverkit "DriverKit download failed"
      return
    fi
    if ! run_root_script driverkit-install "#!/bin/sh
/usr/sbin/installer -pkg '$pkg' -target /
"; then
      step failed karabiner_driverkit "DriverKit /usr/sbin/installer failed"
      return
    fi
  fi
  if [ "$DRY_RUN" = true ]; then
    step skipped karabiner_driverkit "would poll dext approval"
    return
  fi
  # dext approval has no declarative or CLI path (/usr/bin/systemextensionsctl has no
  # approve verb; sysext policy is MDM-only), so the pane is the surface.
  step_from_outcome karabiner_driverkit "$(open_and_poll "DriverKit dext approval" \
    "x-apple.systempreferences:com.apple.LoginItems-Settings.extension" 120 vhid_dext_activated)"
}

# Primary keyboard engine (OUTCOMES.md a-e): Karabiner-Elements. Launching it
# once triggers macOS's own permission prompts for the grabber.
gate_keyboard_engine() {
  if is_headless; then
    step skipped keyboard_engine "keyboard engine skipped (headless)"
    return
  fi
  if [ ! -d /Applications/Karabiner-Elements.app ]; then
    step skipped keyboard_engine "Karabiner-Elements not installed yet (homebrew cask installs it on switch)"
    return
  fi
  if karabiner_grabber_up; then
    step ok keyboard_engine "Karabiner-Elements grabber running"
    return
  fi
  if [ "$DRY_RUN" = true ]; then
    step skipped keyboard_engine "would launch Karabiner-Elements once for OS prompts"
    return
  fi
  echo >&2 "keyboard engine: launching Karabiner-Elements once (macOS shows its own permission prompts)"
  /usr/bin/open -a Karabiner-Elements >/dev/null 2>&1 || true
  if wait_for karabiner_grabber_up 120; then
    step ok keyboard_engine "Karabiner-Elements grabber running (grants accepted)"
  else
    step skipped keyboard_engine "Karabiner-Elements grabber not up within 120s; approve the OS prompts, it converges automatically"
  fi
}

# Karabiner-Elements >= 15 renamed karabiner_grabber to Karabiner-Core-Service.
karabiner_grabber_up() {
  /usr/bin/pgrep -f "Karabiner-Core-Service|karabiner_grabber" >/dev/null 2>&1
}

# Hammerspoon is the process that calls the Accessibility API, so the grant is
# its own. macOS 27 gates the toggle itself behind Touch ID, which is why this
# opens the pane and waits instead of clicking anything.
gate_accessibility() {
  if [ "$DRY_RUN" = true ]; then
    step skipped accessibility "would probe Accessibility and open the pane if missing"
    return
  fi
  if is_headless; then
    step skipped accessibility "Accessibility skipped (headless)"
    return
  fi
  if [ ! -x "$HS_CLI" ]; then
    step skipped accessibility "Hammerspoon not installed yet; the cask installs it on this switch and the next run opens the pane"
    return
  fi
  hammerspoon_running || /usr/bin/open -a Hammerspoon >/dev/null 2>&1 || true
  step_from_outcome accessibility "$(open_and_poll "Accessibility (window placement)" \
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility" 120 hammerspoon_ax_granted)"
}

hammerspoon_running() {
  /usr/bin/pgrep -x Hammerspoon >/dev/null 2>&1
}

hammerspoon_ax_granted() {
  [ "$("$HS_CLI" -c "accessibilityGranted()" 2>/dev/null || true)" = true ]
}

# macOS owns the confirmation panel for the default browser and there is no
# declarative path to it, so this behaves like the TCC gates.
gate_default_browser() {
  if is_headless; then
    step skipped default_browser "default browser skipped (headless)"
    return
  fi
  if [ "$DRY_RUN" = true ]; then
    step skipped default_browser "would ask macOS to make Aside the default browser"
    return
  fi
  local out
  out="$(default_browser_converge "$BROWSER_BUNDLE_ID" 120)"
  step_from_outcome default_browser "$(case "$out" in skipped:*) printf '%s' "$out" ;; *) printf 'ok: %s' "$out" ;; esac)"
}

# Spotlight ⌘Space (symbolic hot key 64) stays imperative on purpose: a
# preference write can only replace the whole AppleSymbolicHotKeys dict, which
# would clobber every other shortcut, so this patches the single entry.
gate_spotlight() {
  if is_headless; then
    step skipped spotlight "Spotlight restore skipped (headless)"
    return
  fi
  if [ "$DRY_RUN" = true ]; then
    step skipped spotlight "would restore Spotlight ⌘Space, Clipboard History, and the terminal alias"
    return
  fi
  step ok spotlight "$(spotlight_restore)"
}

# The Time Machine menu extra is declared in nix-darwin CustomUserPreferences and
# only re-checked here; Cursor tray hiding is app storage, not a preference.
gate_menubar() {
  if is_headless; then
    step skipped menubar "Menu bar restore skipped (headless)"
    return
  fi
  local tm=false cursor=false
  time_machine_hidden && tm=true
  cursor_tray_hidden && cursor=true
  if [ "$tm" = true ] && [ "$cursor" = true ]; then
    step ok menubar "Time Machine + Cursor already hidden from menu bar"
    return
  fi
  if [ "$DRY_RUN" = true ]; then
    step skipped menubar "would hide Time Machine + Cursor menu bar extras"
    return
  fi
  local parts=""
  if [ "$tm" = false ]; then
    hide_time_machine
    parts="Time Machine hidden"
  else
    parts="Time Machine already hidden"
  fi
  if [ "$cursor" = false ]; then
    local out
    out="$(hide_cursor_tray)"
    case "$out" in
      skipped:*) parts="$parts; Cursor skipped: ${out#skipped: }" ;;
      *) parts="$parts; Cursor tray hidden (restart Cursor if still visible)" ;;
    esac
  else
    parts="$parts; Cursor tray already hidden"
  fi
  step ok menubar "$parts"
}
