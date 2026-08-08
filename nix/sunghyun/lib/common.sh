JXA_DIR="@jxaDir@"
HS_CLI="@hsCli@"
IME_ABC="@imeAbc@"
IME_KOREAN="@imeKorean@"
DIA_BUNDLE_ID="@diaBundleId@"
TERMINAL_ALIAS_BUNDLE_ID="@terminalAliasBundleId@"
TERMINAL_ALIAS_TARGET="@terminalAliasTarget@"
KANATA_LABEL="@kanataLabel@"
KANATA_MIN_VERSION="@kanataMinVersion@"
KANATA_DRIVER_URL="@kanataDriverUrl@"
SPOTLIGHT_HOTKEY_ID=64
ACTIVATE_SETTINGS=/System/Library/PrivateFrameworks/SystemAdministration.framework/Resources/activateSettings
CONFIG_DIR="$HOME/.config/sunghyun"
LOG_DIR="$HOME/Library/Logs/sunghyun"

truthy() {
  case "$1" in
    1 | true | TRUE | True | yes | YES | Yes | on | ON) return 0 ;;
    *) return 1 ;;
  esac
}

is_headless() {
  if [ -n "${SUNGHYUN_HEADLESS:-}" ] && truthy "${SUNGHYUN_HEADLESS}"; then
    return 0
  fi
  # No window server session is the macOS definition of headless here; an ssh
  # login and a launchd daemon both report something other than Aqua.
  [ "$(/bin/launchctl managername 2>/dev/null || true)" != Aqua ]
}

jxa() {
  local script="$1"
  shift
  /usr/bin/osascript -l JavaScript "$JXA_DIR/$script.js" "$@"
}

# Steps accumulate as status<TAB>id<TAB>message so a report can be printed in
# either format at the end.
STEPS=""

step() {
  STEPS="$STEPS$1	$2	$3
"
}

step_from_outcome() {
  local id="$1" text="$2"
  case "$text" in
    ok:*) step ok "$id" "${text#ok: }" ;;
    skipped:*) step skipped "$id" "${text#skipped: }" ;;
    failed:*) step failed "$id" "${text#failed: }" ;;
    *) step failed "$id" "$text" ;;
  esac
}

json_escape() {
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

print_report() {
  local format="$1" headless="$2"
  local oks=0 skips=0 fails=0 first=1
  if [ "$format" = json ]; then
    printf '{\n  "headless": %s,\n  "steps": [\n' "$headless"
  else
    printf 'headless=%s\n' "$headless"
  fi
  while IFS='	' read -r status id message; do
    [ -z "$status" ] && continue
    case "$status" in
      ok) oks=$((oks + 1)) ;;
      skipped) skips=$((skips + 1)) ;;
      failed) fails=$((fails + 1)) ;;
    esac
    if [ "$format" = json ]; then
      [ "$first" = 0 ] && printf ',\n'
      first=0
      printf '    { "id": "%s", "status": "%s", "message": "%s" }' \
        "$(json_escape "$id")" "$status" "$(json_escape "$message")"
    else
      printf '[%s] %s: %s\n' "$status" "$id" "$message"
    fi
  done <<EOF
$STEPS
EOF
  if [ "$format" = json ]; then
    printf '\n  ],\n  "summary": { "ok": %s, "skipped": %s, "failed": %s }\n}\n' \
      "$oks" "$skips" "$fails"
  else
    printf 'summary ok=%s skipped=%s failed=%s\n' "$oks" "$skips" "$fails"
  fi
  [ "$fails" -eq 0 ]
}

defaults_read() {
  local out
  out="$(/usr/bin/defaults read "$@" 2>/dev/null)" || return 1
  [ -n "$out" ] || return 1
  printf '%s' "$out"
}

# A nested value out of a preference domain as a raw scalar. `defaults read`
# cannot address a key path and its old-style output loses types.
defaults_extract() {
  /usr/bin/defaults export "$1" - 2>/dev/null |
    /usr/bin/plutil -extract "$2" raw -o - - 2>/dev/null
}

have() {
  command -v "$1" >/dev/null 2>&1
}

first_existing() {
  local candidate
  for candidate in "$@"; do
    if [ -e "$candidate" ]; then
      printf '%s' "$candidate"
      return 0
    fi
  done
  return 1
}

# Open a Settings pane once, then poll until the probe passes or the budget runs
# out. A timeout is a graceful skip: the next switch reopens the pane. The owner
# flipping the toggle in the opened window is the entire human surface.
open_and_poll() {
  local what="$1" pane="$2" budget="$3" probe="$4"
  if "$probe"; then
    echo "ok: $what already granted"
    return 0
  fi
  if is_headless; then
    echo "skipped: $what skipped (headless); converges later"
    return 0
  fi
  echo >&2 "$what: opening System Settings pane; waiting for the toggle (no prompts)"
  /usr/bin/open "$pane" >/dev/null 2>&1 || true
  local waited=0
  while [ "$waited" -lt "$budget" ]; do
    sleep 3
    waited=$((waited + 3))
    if "$probe"; then
      echo "ok: $what granted"
      return 0
    fi
  done
  echo "skipped: $what not granted within ${budget}s; skipping (the next darwin-rebuild switch reopens this pane on its own)"
}
