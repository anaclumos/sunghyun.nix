KANATA_PLIST="/Library/LaunchDaemons/$KANATA_LABEL.plist"
KANATA_PLIST_DISABLED="/Library/LaunchDaemons/$KANATA_LABEL.plist.disabled"
VHID_DAEMON_LABEL="org.pqrs.service.daemon.Karabiner-VirtualHIDDevice-Daemon"
VHID_DAEMON="/Library/Application Support/org.pqrs/Karabiner-DriverKit-VirtualHIDDevice/Applications/Karabiner-VirtualHIDDevice-Daemon.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Daemon"
VHID_MANAGER="/Applications/.Karabiner-VirtualHIDDevice-Manager.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Manager"
KANATA_TEMP_OUT=/tmp/sunghyun-kanata-temp.out
KANATA_TEMP_ERR=/tmp/sunghyun-kanata-temp.err

# kanata log lines that prove the grab and the output loop are up.
SUCCESS_MARKERS="entering the processing loop
keyboard grabbed, entering event processing loop"

# Permission and driver states that never self-heal inside a stage; this is
# exactly the brick class (a grab with no healthy output).
FATAL_MARKERS="Input Monitoring permission is denied
Input Monitoring permission not yet decided
Accessibility permission
IOHIDDeviceOpen error
not permitted
grab failed
driver is not activated
Couldn't register any device"

# Output-backend loss after a successful start.
DEGRADED_MARKERS="connect_failed
output backend not ready
output backend unavailable
DriverKit virtual keyboard not ready"

cmd_kanata() {
  case "${1:-status}" in
    status) kanata_status ;;
    disable) kanata_disable ;;
    enable)
      shift
      if [ "${1:-}" != --safe ]; then
        die "refusing: use \`sunghyun kanata enable --safe\` (passthrough proof + rollback)"
      fi
      kanata_enable_safe
      ;;
    *) die "usage: sunghyun kanata [status|disable|enable --safe]" ;;
  esac
}

kanata_pids() {
  /usr/bin/pgrep -x kanata 2>/dev/null || true
}

kanata_running() {
  [ -n "$(kanata_pids)" ]
}

vhid_daemon_running() {
  /usr/bin/pgrep -f Karabiner-VirtualHIDDevice-Daemon >/dev/null 2>&1
}

vhid_dext_activated() {
  /usr/bin/systemextensionsctl list 2>/dev/null |
    grep "org.pqrs.Karabiner-DriverKit-VirtualHIDDevice" |
    grep -q activated
}

# The VirtualHID keyboard appears in the HID device tree only while a client is
# connected, so its presence proves the output path exists.
vhid_output_device_present() {
  /usr/bin/hidutil list 2>/dev/null | grep -qE "VirtualHIDKeyboard|Karabiner DriverKit"
}

physical_keyboard_present() {
  /usr/bin/hidutil list 2>/dev/null | grep Keyboard | grep -qv VirtualHID
}

kanata_status() {
  local pids state
  pids="$(kanata_pids | tr '\n' ',' | sed 's/,$//')"
  if [ -z "$pids" ]; then
    if [ -f "$KANATA_PLIST" ] || [ -f "$KANATA_PLIST_DISABLED" ]; then
      state=Disabled
    else
      state=Absent
    fi
  elif /bin/launchctl print "system/$KANATA_LABEL" >/dev/null 2>&1; then
    state=RunningDaemon
  else
    state=RunningOrphan
  fi
  echo "kanata_state=$state"
  echo "plist_active=$([ -f "$KANATA_PLIST" ] && echo true || echo false)"
  echo "plist_disabled=$([ -f "$KANATA_PLIST_DISABLED" ] && echo true || echo false)"
  echo "pids=$pids"
  echo "vhid_daemon_running=$(vhid_daemon_running && echo true || echo false)"
  echo "vhid_dext_activated=$(vhid_dext_activated && echo true || echo false)"
  echo "vhid_output_device_present=$(vhid_output_device_present && echo true || echo false)"
  if [ -z "$pids" ]; then
    echo "input_monitoring=unknown (kanata not running; TCC not readable without FDA)"
  else
    echo "input_monitoring=granted (kanata process is up and holding the grab)"
  fi
}

# One privileged script per sequence, so a single cached ticket (or a single
# owner-typed password) covers it. Never /usr/bin/osascript admin, never a prompt loop.
run_root_script() {
  local name="$1" body="$2" path
  path="$(mktemp "/tmp/sunghyun-$name.XXXXXX.sh")"
  printf '%s' "$body" >"$path"
  chmod 755 "$path"
  if /usr/bin/sudo -n true 2>/dev/null; then
    /usr/bin/sudo -n /bin/sh "$path"
  else
    /usr/bin/sudo /bin/sh "$path"
  fi
}

# The /bin/launchctl disable override means a bare plist rename back can never re-arm
# the daemon on boot without the safe-enable gate, which runs `/bin/launchctl enable`
# itself.
kanata_disable() {
  run_root_script kanata-disable "#!/bin/sh
/bin/launchctl bootout system/$KANATA_LABEL 2>/dev/null || true
/bin/launchctl disable system/$KANATA_LABEL 2>/dev/null || true
/usr/bin/pkill -x kanata 2>/dev/null || true
if [ -f '$KANATA_PLIST' ]; then mv -f '$KANATA_PLIST' '$KANATA_PLIST_DISABLED'; fi
exit 0
"
  /usr/bin/pkill -x kanata 2>/dev/null || true
  echo >&2 "kanata disabled (daemon bootout; /bin/launchctl disable override; plist -> .disabled if present)"
}

emergency_rollback() {
  echo >&2 "kanata: ROLLBACK — disabling after failed proof"
  kanata_disable || true
}

resolve_kanata_bin() {
  local candidate
  for candidate in /opt/homebrew/bin/kanata /usr/local/bin/kanata /run/current-system/sw/bin/kanata; do
    if [ -f "$candidate" ]; then
      printf '%s' "$candidate"
      return 0
    fi
  done
  command -v kanata 2>/dev/null
}

# kanata < 1.12.0 predates the grab-without-output recovery fix, which is
# exactly today's brick class.
ensure_kanata_min_version() {
  local bin="$1" text version
  text="$("$bin" --version 2>&1 || true)"
  version="$(printf '%s' "$text" | tr ' ' '\n' | sed -n 's/^v\{0,1\}\([0-9]\{1,\}\.[0-9]\{1,\}\.[0-9]\{1,\}\).*/\1/p' | head -1)"
  [ -n "$version" ] || die "cannot parse kanata version from: $text"
  local lowest
  lowest="$(printf '%s\n%s\n' "$version" "$KANATA_MIN_VERSION" | sort -t. -k1,1n -k2,2n -k3,3n | head -1)"
  if [ "$lowest" != "$KANATA_MIN_VERSION" ] && [ "$version" != "$KANATA_MIN_VERSION" ]; then
    die "kanata $version < $KANATA_MIN_VERSION lacks the grab-without-output recovery fix (brick risk); upgrade first"
  fi
}

ensure_vhid_stack() {
  if ! vhid_dext_activated; then
    [ -f "$VHID_MANAGER" ] ||
      die "Karabiner-DriverKit-VirtualHIDDevice missing; install the v6.2.0 pkg first"
    echo >&2 "kanata: activating Karabiner VirtualHID dext"
    run_root_script kanata-vhid-activate "#!/bin/sh
'$VHID_MANAGER' forceActivate
exit 0
" || true
  fi
  if ! vhid_daemon_running; then
    echo >&2 "kanata: starting Karabiner-VirtualHIDDevice-Daemon"
    run_root_script kanata-vhid-kickstart "#!/bin/sh
/bin/launchctl kickstart system/$VHID_DAEMON_LABEL 2>/dev/null || true
exit 0
" || true
    wait_for vhid_daemon_running 5
  fi
  if ! vhid_daemon_running && [ -f "$VHID_DAEMON" ]; then
    run_root_script kanata-vhid-daemon-start "#!/bin/sh
nohup '$VHID_DAEMON' >/dev/null 2>&1 &
exit 0
" || true
    wait_for vhid_daemon_running 5
  fi
  vhid_daemon_running ||
    die "VirtualHID daemon not running; kanata would brick the keyboard (refusing enable)"
}

wait_for() {
  local probe="$1" budget="$2" waited=0
  while [ "$waited" -lt "$budget" ]; do
    "$probe" && return 0
    sleep 1
    waited=$((waited + 1))
  done
  "$probe"
}

marker_in() {
  local haystack="$1" markers="$2" marker
  while IFS= read -r marker; do
    [ -n "$marker" ] || continue
    case "$haystack" in
      *"$marker"*)
        printf '%s' "$marker"
        return 0
        ;;
    esac
  done <<EOF
$markers
EOF
  return 1
}

# Bytes appended to the marked logs since the mark was taken.
log_tail_since() {
  local path offset
  while IFS=: read -r path offset; do
    [ -n "$path" ] || continue
    [ -f "$path" ] || continue
    tail -c "+$((offset + 1))" "$path" 2>/dev/null || true
  done <<EOF
$1
EOF
}

log_marks() {
  local path
  for path in "$@"; do
    if [ -f "$path" ]; then
      printf '%s:%s\n' "$path" "$(wc -c <"$path" | tr -d ' ')"
    else
      printf '%s:0\n' "$path"
    fi
  done
}

# Baseline before any grab: a physical keyboard exists and nothing is already
# seizing it. Needs no /usr/bin/sudo and no owner typing.
prove_baseline() {
  if ! physical_keyboard_present; then
    die "keyboard stack unhealthy before kanata enable (no physical keyboard visible in /usr/bin/hidutil list); refusing to grab keyboard"
  fi
  if kanata_running; then
    die "keyboard stack unhealthy before kanata enable (kanata already running; disable first); refusing to grab keyboard"
  fi
}

# A started kanata is healthy when a success marker shows up in its fresh log
# inside the budget, no fatal marker appears, the pid is stable, and the
# VirtualHID output device is present.
prove_kanata_stage() {
  local marks="$1" budget="$2" waited=0 combined marker
  while :; do
    combined="$(log_tail_since "$marks")"
    if marker="$(marker_in "$combined" "$FATAL_MARKERS")"; then
      echo "kanata log shows fatal condition: $marker"
      return 1
    fi
    if marker_in "$combined" "$SUCCESS_MARKERS" >/dev/null; then
      break
    fi
    if ! kanata_running; then
      echo "kanata exited before reaching the processing loop; log tail: $(printf '%s' "$combined" | tail -c 400)"
      return 1
    fi
    if [ "$waited" -ge "$budget" ]; then
      echo "no grab/processing-loop marker within ${budget}s; log tail: $(printf '%s' "$combined" | tail -c 400)"
      return 1
    fi
    sleep 1
    waited=$((waited + 1))
  done

  if ! wait_for vhid_output_device_present 8; then
    echo "VirtualHID output keyboard did not appear in /usr/bin/hidutil list"
    return 1
  fi

  local pids_a pids_b
  pids_a="$(kanata_pids)"
  [ -n "$pids_a" ] || {
    echo "kanata exited right after reporting the processing loop"
    return 1
  }
  sleep 2
  pids_b="$(kanata_pids)"
  if [ "$pids_a" != "$pids_b" ]; then
    echo "kanata pid churn (restart loop?): $pids_a -> $pids_b"
    return 1
  fi

  # connect_failed lines before the success marker are startup retries; the same
  # line after it means the output backend degraded.
  if marker="$(marker_in "$(log_after_success "$marks")" "$DEGRADED_MARKERS")"; then
    echo "kanata output backend degraded after start: $marker"
    return 1
  fi
  return 0
}

log_after_success() {
  local fresh marker
  fresh="$(log_tail_since "$1")"
  while IFS= read -r marker; do
    [ -n "$marker" ] || continue
    case "$fresh" in
      *"$marker"*)
        printf '%s' "${fresh##*"$marker"}"
        return 0
        ;;
    esac
  done <<EOF
$SUCCESS_MARKERS
EOF
  printf '%s' "$fresh"
}

# Post-enable watchdog: after the settle window the same pid must still be alive
# and no fatal or degraded marker may have appeared.
watchdog_recheck() {
  local marks="$1" settle="$2" pids_before pids_after marker after
  pids_before="$(kanata_pids)"
  sleep "$settle"
  pids_after="$(kanata_pids)"
  [ -n "$pids_after" ] || {
    echo "kanata died within the watchdog window"
    return 1
  }
  if [ "$pids_before" != "$pids_after" ]; then
    echo "kanata restarted within the watchdog window: $pids_before -> $pids_after"
    return 1
  fi
  after="$(log_after_success "$marks")"
  if marker="$(marker_in "$after" "$FATAL_MARKERS")"; then
    echo "fatal condition during watchdog window: $marker"
    return 1
  fi
  if marker="$(marker_in "$after" "$DEGRADED_MARKERS")"; then
    echo "output backend degraded during watchdog window: $marker"
    return 1
  fi
  return 0
}

stop_temp_kanata() {
  run_root_script kanata-stop "#!/bin/sh
/usr/bin/pkill -x kanata 2>/dev/null || true
exit 0
" >/dev/null 2>&1 || true
  /usr/bin/pkill -x kanata 2>/dev/null || true
  local waited=0
  while [ "$waited" -lt 3 ] && kanata_running; do
    sleep 1
    waited=$((waited + 1))
  done
}

start_kanata_temp() {
  local bin="$1" cfg="$2"
  stop_temp_kanata
  rm -f "$KANATA_TEMP_OUT" "$KANATA_TEMP_ERR"
  # Root is required for the VirtualHID IPC under tmp/rootonly; nohup detaches
  # so /usr/bin/sudo never blocks on it.
  run_root_script kanata-temp-start "#!/bin/sh
/bin/launchctl bootout system/$KANATA_LABEL 2>/dev/null || true
nohup '$bin' --cfg '$cfg' --no-wait >$KANATA_TEMP_OUT 2>$KANATA_TEMP_ERR &
exit 0
" || return 1
  sleep 2
  kanata_running || {
    printf '%s' "$(cat "$KANATA_TEMP_ERR" 2>/dev/null || true)"
    return 1
  }
  return 0
}

# Start kanata outside launchd on one config, prove health, stop it.
kanata_run_stage() {
  local bin="$1" cfg="$2" label="$3" err marks proof
  if ! err="$(start_kanata_temp "$bin" "$cfg")"; then
    case "$err" in
      *"Input Monitoring"* | *"not permitted"*) ;;
      *)
        emergency_rollback
        die "$label start failed; rolled back: $err"
        ;;
    esac
    if is_headless; then
      emergency_rollback
      skip "$label needs the Input Monitoring grant for $bin (headless; converge on next run)"
    fi
    echo >&2 "kanata: Input Monitoring grant missing for $bin; opening System Settings and waiting for the toggle"
    /usr/bin/open "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent" >/dev/null 2>&1 || true
    local waited=0 started=0
    while [ "$waited" -lt 120 ]; do
      sleep 4
      waited=$((waited + 4))
      if err="$(start_kanata_temp "$bin" "$cfg")"; then
        started=1
        break
      fi
    done
    if [ "$started" = 0 ]; then
      emergency_rollback
      die "$label start failed (often Input Monitoring); rolled back: $err"
    fi
  fi
  marks="$(printf '%s:0\n%s:0\n' "$KANATA_TEMP_OUT" "$KANATA_TEMP_ERR")"
  if ! proof="$(prove_kanata_stage "$marks" 20)"; then
    emergency_rollback
    die "$label proof failed; rolled back: $proof"
  fi
  stop_temp_kanata
}

kanata_launch_daemon_plist() {
  local bin="$1" cfg="$2"
  cat <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>$KANATA_LABEL</string>
	<key>ProgramArguments</key>
	<array>
		<string>$bin</string>
		<string>--cfg</string>
		<string>$cfg</string>
		<string>--no-wait</string>
	</array>
	<key>UserName</key>
	<string>root</string>
	<key>EnvironmentVariables</key>
	<dict>
		<key>PATH</key>
		<string>/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
	</dict>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>StandardOutPath</key>
	<string>$LOG_DIR/kanata.out.log</string>
	<key>StandardErrorPath</key>
	<string>$LOG_DIR/kanata.err.log</string>
</dict>
</plist>
PLIST
}

kanata_install_launch_daemon() {
  local bin="$1" cfg="$2" staged
  mkdir -p "$LOG_DIR" "$CONFIG_DIR"
  staged="$CONFIG_DIR/$KANATA_LABEL.plist"
  kanata_launch_daemon_plist "$bin" "$cfg" >"$staged"
  stop_temp_kanata
  run_root_script kanata-launchd-install "#!/bin/sh
set -e
/bin/launchctl bootout system/$KANATA_LABEL 2>/dev/null || true
/bin/launchctl enable system/$KANATA_LABEL
rm -f '$KANATA_PLIST_DISABLED'
cp -f '$staged' '$KANATA_PLIST'
chown root:wheel '$KANATA_PLIST'
chmod 644 '$KANATA_PLIST'
/bin/launchctl bootstrap system '$KANATA_PLIST'
"
}

kanata_enable_safe() {
  if is_headless; then
    skip "kanata enable --safe skipped (headless; no keyboard session to prove against)"
  fi
  # One ticket for the whole run: the privileged steps then use /usr/bin/sudo -n.
  /usr/bin/sudo -v

  echo >&2 "kanata: baseline keyboard-stack proof"
  prove_baseline
  ensure_vhid_stack

  local passthrough full bin marks proof
  passthrough="$CONFIG_DIR/kanata-passthrough.kbd"
  full="$CONFIG_DIR/kanata.kbd"
  [ -f "$passthrough" ] || die "$passthrough missing; darwin-rebuild switch materializes it"
  [ -f "$full" ] || die "$full missing; darwin-rebuild switch materializes it"

  bin="$(resolve_kanata_bin)"
  [ -n "$bin" ] || die "kanata binary not found (brew install kanata / flake homebrew)"
  ensure_kanata_min_version "$bin"
  "$bin" --cfg "$passthrough" --check || die "kanata --check failed for $passthrough"
  "$bin" --cfg "$full" --check || die "kanata --check failed for $full"

  echo >&2 "kanata: starting passthrough stage with rollback watchdog"
  kanata_run_stage "$bin" "$passthrough" passthrough

  echo >&2 "kanata: starting full-config stage with rollback watchdog"
  kanata_run_stage "$bin" "$full" full

  echo >&2 "kanata: installing LaunchDaemon"
  marks="$(log_marks "$LOG_DIR/kanata.out.log" "$LOG_DIR/kanata.err.log")"
  if ! kanata_install_launch_daemon "$bin" "$full"; then
    emergency_rollback
    die "LaunchDaemon install failed; rolled back"
  fi
  if ! proof="$(prove_kanata_stage "$marks" 20)"; then
    emergency_rollback
    die "LaunchDaemon proof failed; rolled back: $proof"
  fi
  if ! proof="$(watchdog_recheck "$marks" 10)"; then
    emergency_rollback
    die "LaunchDaemon watchdog failed; rolled back: $proof"
  fi
  echo "kanata: enabled safely (passthrough + full + launchd proofs passed)"
}
