# shellcheck shell=bash

# Test the controlling terminal, not stdin: a piped or redirected stdin says
# nothing about whether sudo can prompt, and sudo reads from /dev/tty.
sudo_has_controlling_tty() { { : < /dev/tty; } 2>/dev/null; }

sudo_keepalive_start() {
  if sudo -n true 2>/dev/null; then
    :
  elif sudo_has_controlling_tty; then
    echo "sudo: enter password once for privileged steps" >&2
    sudo -v < /dev/tty || return 1
  else
    echo "sudo: no credential cache and no controlling terminal for sudo -v" >&2
    return 1
  fi

  # `|| true` so an inherited set -e cannot kill the refresher when -n misses.
  while true; do
    sudo -n true 2>/dev/null || true
    sleep 60
    kill -0 "$$" || exit 0
  done &
  SUDO_KEEPALIVE_PID=$!
  export SUDO_KEEPALIVE_PID
}

sudo_keepalive_stop() {
  if [[ -n "${SUDO_KEEPALIVE_PID:-}" ]]; then
    kill "$SUDO_KEEPALIVE_PID" 2>/dev/null || true
    unset SUDO_KEEPALIVE_PID
  fi
}

# Plain sudo after the -n probe, so a missing or expired cache can prompt once
# instead of hard-failing.
sudo_do() {
  if sudo -n true 2>/dev/null; then
    sudo -n "$@"
  else
    sudo "$@"
  fi
}
