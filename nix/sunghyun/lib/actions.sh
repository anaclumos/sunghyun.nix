TILE_ACTIONS="@tileActions@"

resolve_app() {
  case "$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')" in
@appCases@
    *) return 1 ;;
  esac
}

resolve_tile() {
  local name
  name="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  case "$name" in
@tileAliasCases@
  esac
  case " $TILE_ACTIONS " in
    *" $name "*) printf '%s' "$name" ;;
    *) return 1 ;;
  esac
}

resolve_ime() {
  case "$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')" in
    abc | english | en) printf '%s' "$IME_ABC" ;;
    korean | 2set | 2setkorean | ko) printf '%s' "$IME_KOREAN" ;;
    *.*) printf '%s' "$1" ;;
    *) return 1 ;;
  esac
}

cmd_open() {
  local target="${1:-}"
  [ -n "$target" ] || die "open requires a target"
  case "$(printf '%s' "$target" | tr '[:upper:]' '[:lower:]')" in
    browser | default-browser | default_browser)
      cmd_open_default_browser
      return
      ;;
  esac
  local bundle_id
  if bundle_id="$(resolve_app "$target")"; then
    exec /usr/bin/open -b "$bundle_id"
  fi
  case "$target" in
    *.*) exec /usr/bin/open -b "$target" ;;
    *) exec /usr/bin/open -a "$target" ;;
  esac
}

cmd_open_default_browser() {
  if is_headless; then
    skip "default browser skipped in headless (no GUI session)"
  fi
  local handler
  handler="$(jxa default-browser status)"
  handler="${handler#default_browser=}"
  if [ -n "$handler" ] && [ "$handler" != unknown ]; then
    exec /usr/bin/open -b "$handler"
  fi
  exec /usr/bin/open "https://"
}

# Tiling drives the macOS Accessibility API against the frontmost app, so it
# runs inside Hammerspoon: one long-lived, granted process instead of a grant
# per short-lived caller.
cmd_tile() {
  local action
  action="$(resolve_tile "${1:-}")" || die "unknown tile action: ${1:-}"
  if is_headless; then
    skip "tile $action skipped (headless)"
  fi
  if [ ! -x "$HS_CLI" ]; then
    die "Hammerspoon is not installed; the hammerspoon cask installs it on the next switch"
  fi
  local out
  if ! out="$("$HS_CLI" -c "tile(\"$action\")" 2>&1)"; then
    die "Hammerspoon is not reachable ($out); it starts at login and on switch"
  fi
  case "$out" in
    ok:*) printf '%s\n' "${out#ok: }" ;;
    skipped:*) skip "${out#skipped: }" ;;
    failed:*) die "${out#failed: }" ;;
    *) die "$out" ;;
  esac
}

cmd_input_source() {
  local id
  id="$(resolve_ime "${1:-}")" || die "unknown input source: ${1:-}"
  local out
  if ! out="$(jxa input-source "$id" 2>&1)"; then
    if is_headless; then
      skip "$out"
    fi
    die "$out"
  fi
  printf '%s\n' "$out"
}

cmd_toggle_dark_mode() {
  if is_headless; then
    skip "appearance skipped in headless (no window server)"
  fi
  jxa appearance toggle
}

cmd_hotkeys() {
  case "${1:-status}" in
    status)
      if is_headless; then
        skip "reserved chords skipped in headless (no window server)"
      fi
      jxa hotkeys status
      ;;
    apply)
      if is_headless; then
        skip "reserved chords skipped in headless (no window server)"
      fi
      hotkeys_apply
      ;;
    *) die "usage: sunghyun hotkeys [status|apply]" ;;
  esac
}

# Two writes, neither of which does the other's job: the running window server
# never re-reads the preference domain, and a fresh login only reads that
# domain. `-dict-add` patches the single offending identifier, so every other
# system shortcut in AppleSymbolicHotKeys survives.
hotkeys_apply() {
  local scanned
  scanned="$(jxa hotkeys scan)"
  local id key_equivalent virtual_key modifiers enabled
  while read -r id key_equivalent virtual_key modifiers enabled; do
    [ -n "$id" ] || continue
    [ "$enabled" = true ] || continue
    /usr/bin/defaults write com.apple.symbolichotkeys AppleSymbolicHotKeys -dict-add "$id" \
      "<dict><key>enabled</key><false/><key>value</key><dict><key>parameters</key><array><integer>$key_equivalent</integer><integer>$virtual_key</integer><integer>$modifiers</integer></array><key>type</key><string>standard</string></dict></dict>"
  done <<EOF
$scanned
EOF
  jxa hotkeys disable
}

cmd_fn_state() {
  case "${1:-status}" in
    status)
      local mode
      mode="$(fn_state_mode)" || skip "IOHIDSystem does not report HIDFKeyMode here"
      if [ "$mode" = 0 ]; then
        echo "standard_function_keys=false"
      else
        echo "standard_function_keys=true"
      fi
      ;;
    apply)
      shift
      local wanted="${1:-}"
      case "$wanted" in
        true | false) ;;
        *) die "usage: sunghyun fn-state apply <true|false>" ;;
      esac
      fn_state_apply "$wanted"
      echo "standard_function_keys=$wanted"
      ;;
    *) die "usage: sunghyun fn-state [status|apply <true|false>]" ;;
  esac
}

fn_state_mode() {
  local mode
  mode="$(/usr/sbin/ioreg -c IOHIDSystem -r -d1 2>/dev/null |
    sed -n 's/.*"HIDFKeyMode"=\([0-9]*\).*/\1/p' | head -1)"
  [ -n "$mode" ] || return 1
  printf '%s' "$mode"
}

# `system.defaults` writes the preference, which IOHIDSystem only reads at
# login; activateSettings is what makes a running session pick it up.
fn_state_apply() {
  /usr/bin/defaults write -g com.apple.keyboard.fnState -bool "$1"
  [ -x "$ACTIVATE_SETTINGS" ] && "$ACTIVATE_SETTINGS" -u
  return 0
}

cmd_default_browser() {
  case "${1:-status}" in
    status) jxa default-browser status ;;
    set)
      shift
      local bundle_id="${1:-$DIA_BUNDLE_ID}" budget="${2:-120}"
      local out
      out="$(default_browser_converge "$bundle_id" "$budget")"
      case "$out" in
        skipped:*) skip "${out#skipped: }" ;;
        *) printf '%s\n' "$out" ;;
      esac
      ;;
    *) die "usage: sunghyun default-browser [status|set [bundle-id] [timeout]]" ;;
  esac
}

default_browser_current() {
  local out
  out="$(jxa default-browser status)"
  printf '%s' "${out#default_browser=}"
}

default_browser_converge() {
  local bundle_id="$1" budget="$2"
  if [ "$(default_browser_current)" = "$bundle_id" ]; then
    echo "$bundle_id is already the default browser"
    return 0
  fi
  if is_headless; then
    echo "skipped: default browser skipped (headless; the panel needs a GUI session)"
    return 0
  fi
  if ! jxa default-browser installed "$bundle_id" >/dev/null 2>&1; then
    echo "skipped: $bundle_id is not installed yet; the cask installs it on this switch and the next run sets it"
    return 0
  fi
  jxa default-browser set "$bundle_id" >/dev/null
  local waited=0
  while [ "$waited" -lt "$budget" ]; do
    sleep 2
    waited=$((waited + 2))
    if [ "$(default_browser_current)" = "$bundle_id" ]; then
      echo "$bundle_id is now the default browser"
      return 0
    fi
  done
  echo "skipped: default browser still $(default_browser_current); macOS's confirmation panel was not answered within ${budget}s"
}

cmd_virt() {
  local hv model reasons=""
  hv="$(/usr/sbin/sysctl -n kern.hv_vmm_present 2>/dev/null || true)"
  model="$(/usr/sbin/sysctl -n hw.model 2>/dev/null || true)"
  [ "$hv" = 1 ] && reasons="kern.hv_vmm_present=1"
  case "$model" in
    VirtualMac*)
      [ -n "$reasons" ] && reasons="$reasons, "
      reasons="${reasons}hw.model=$model"
      ;;
  esac
  if [ -n "$reasons" ]; then
    echo "virtual machine ($reasons); App Store / mas surfaces skip by design"
    return 0
  fi
  echo "physical machine (hw.model=${model:-unknown})"
  return 1
}
