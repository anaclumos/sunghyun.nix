# shellcheck shell=bash
# Source from privileged setup/uninstall scripts:
#   source "$(dirname "$0")/sudo-keepalive.sh"
#   sudo_keepalive_start || exit 1
#   trap sudo_keepalive_stop EXIT ERR INT TERM
#
# Pattern: interactive sudo -v once, then refresh with sudo -n every 60s.
# Does not log or store the password.
#
# Keep-alive probes use sudo -n only. Destructive callers should use plain
# sudo (or sudo_do) so a flaky timestamp can re-prompt instead of hard-failing.

sudo_keepalive_start() {
  if sudo -n true 2>/dev/null; then
    :
  elif [[ -t 0 ]]; then
    echo "sudo: enter password once for privileged steps" >&2
    sudo -v || return 1
  else
    echo "sudo: no credential cache and no TTY for sudo -v" >&2
    return 1
  fi

  # $$ is the sourcing shell (bash keeps $$ stable in subshells).
  # || true so inherited set -e cannot kill the refresher when -n misses
  # (e.g. tty_tickets / no controlling TTY in the background job).
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

# Run a privileged command. Prefer a cached ticket via -n probe, then plain
# sudo so a missing/expired cache can prompt once instead of hard-failing.
sudo_do() {
  if sudo -n true 2>/dev/null; then
    sudo -n "$@"
  else
    sudo "$@"
  fi
}
