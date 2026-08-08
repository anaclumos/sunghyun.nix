#!/usr/bin/env bash
#
#   curl -fsSL https://raw.githubusercontent.com/anaclumos/sunghyun.nix/main/install.sh | bash
#
set -euo pipefail

REPO_URL="${SUNGHYUN_REPO_URL:-https://github.com/anaclumos/sunghyun.nix.git}"
REPO_DIR="${SUNGHYUN_DIR:-$HOME/Developer/sunghyun.nix}"
# Must stay `default`: it is the only darwin config that sets no
# ComputerName/LocalHostName, so an unknown Mac keeps its own identity.
DEFAULT_HOST="${SUNGHYUN_HOST:-default}"
export PATH="${HOME}/.local/bin:/nix/var/nix/profiles/default/bin:/run/current-system/sw/bin:/opt/homebrew/bin:/usr/local/bin:${PATH}"

log() { printf 'sunghyun-install: %s\n' "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

# The run is unattended after the single sudo password, so git must never stop
# to ask for credentials on a repo it cannot read.
export GIT_TERMINAL_PROMPT=0

# Test the controlling terminal, not stdin: under `curl ... | bash` stdin is the
# pipe, so `-t 0` is false even in Terminal, and sudo reads /dev/tty anyway.
has_controlling_tty() { { : < /dev/tty; } 2>/dev/null; }

# Homebrew runs `sudo --reset-timestamp` on every invocation with no opt-out,
# and brew runs twice here, so a keep-alive loop cannot hold the ticket: the
# owner's single authorization is held in a sudoers drop-in for the run and
# released on the way out. A grant leaked by a SIGKILL is reclaimed by the next
# run, which reinstalls and releases the same path.
SUDO_DROPIN="/etc/sudoers.d/zz-sunghyun-install"
SUDO_GRANT_HELD=""

sudo_grant_release() {
  [[ -n "${SUDO_GRANT_HELD}" ]] || return 0
  SUDO_GRANT_HELD=""
  sudo rm -f "${SUDO_DROPIN}" 2>/dev/null || true
  log "released the temporary sudo grant (${SUDO_DROPIN})"
}

sudo_grant_acquire() {
  local tmp root_group
  tmp="$(mktemp)"
  # root's primary group is `wheel` on macOS and `root` on Debian/Ubuntu.
  root_group="$(id -gn root 2>/dev/null || echo wheel)"
  printf '%s ALL=(ALL) NOPASSWD: ALL\n' "$(id -un)" >"${tmp}"
  if sudo visudo -cqf "${tmp}" 2>/dev/null &&
     sudo install -m 0440 -o root -g "${root_group}" "${tmp}" "${SUDO_DROPIN}"; then
    SUDO_GRANT_HELD=1
    log "holding a temporary sudo grant for this run (${SUDO_DROPIN}); released on exit"
  else
    log "WARNING: could not install ${SUDO_DROPIN}; sudo may ask again after a brew step"
  fi
  rm -f "${tmp}"
}

sudo_keepalive_start() {
  if [[ "$(id -u)" -eq 0 ]]; then
    log "running as root; no sudo authorization needed"
    return 0
  fi
  command -v sudo >/dev/null 2>&1 || die "sudo is required (or run this as root)"
  if sudo -n true 2>/dev/null; then
    :
  elif has_controlling_tty; then
    log "sudo: enter password once; install continues unattended after this"
    sudo -v < /dev/tty || die "sudo -v failed"
  else
    die "sudo needs a password once but there is no controlling terminal; run this from Terminal"
  fi
  sudo_grant_acquire
  (
    while true; do
      sudo -n true 2>/dev/null || true
      sleep 60
      kill -0 "$$" 2>/dev/null || exit 0
    done
  ) &
  SUDO_KEEPALIVE_PID=$!
  trap 'kill "${SUDO_KEEPALIVE_PID}" 2>/dev/null || true; sudo_grant_release' EXIT INT TERM
}

source_nix_daemon() {
  # shellcheck disable=SC1091
  if [[ -f /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh ]]; then
    # shellcheck source=/dev/null
    . /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
  elif [[ -f "${HOME}/.nix-profile/etc/profile.d/nix.sh" ]]; then
    # shellcheck source=/dev/null
    . "${HOME}/.nix-profile/etc/profile.d/nix.sh"
  fi
  export PATH="/nix/var/nix/profiles/default/bin:/run/current-system/sw/bin:${HOME}/.nix-profile/bin:${PATH}"
}

detect_headless() {
  if [[ "${SUNGHYUN_HEADLESS:-}" == "1" ]]; then
    return 0
  fi
  if [[ "$(uname -s)" != "Darwin" ]]; then
    return 0
  fi
  # managername is Aqua only in a GUI login session.
  if launchctl managername 2>/dev/null | grep -qi Aqua; then
    return 1
  fi
  return 0
}

resolve_flake_host() {
  local host="${SUNGHYUN_HOST:-}"
  if [[ -n "${host}" ]]; then
    printf '%s\n' "${host}"
    return
  fi
  local local_host
  local_host="$(scutil --get LocalHostName 2>/dev/null || true)"
  if [[ -n "${local_host}" ]] && [[ -f "${REPO_DIR}/nix/darwin/hosts/${local_host}.nix" ]]; then
    printf '%s\n' "${local_host}"
    return
  fi
  printf '%s\n' "${DEFAULT_HOST}"
}

# --- shared steps -------------------------------------------------------------
ensure_nix() {
  source_nix_daemon
  if command -v nix >/dev/null 2>&1; then
    log "Nix already installed"
    return 0
  fi
  log "installing Determinate Nix (noninteractive --no-confirm)"
  # Official: https://github.com/DeterminateSystems/nix-installer
  curl --proto '=https' --tlsv1.2 -fsSL https://install.determinate.systems/nix \
    | sh -s -- install --no-confirm
  source_nix_daemon
  command -v nix >/dev/null 2>&1 || die "nix missing after Determinate install"
}

clone_or_update() {
  local url="$1" dir="$2"
  mkdir -p "$(dirname "${dir}")"
  if [[ -d "${dir}/.git" ]]; then
    log "updating ${dir}"
    git -C "${dir}" pull --ff-only || git -C "${dir}" fetch --all --prune
  else
    log "cloning ${url} → ${dir}"
    git clone "${url}" "${dir}"
  fi
}

ensure_repo() { clone_or_update "${REPO_URL}" "${REPO_DIR}"; }

ensure_xcode_clt() {
  if xcode-select -p >/dev/null 2>&1; then
    log "Xcode CLT already present"
    return 0
  fi
  log "installing Xcode Command Line Tools (noninteractive when possible)"
  local marker="/tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress"
  sudo touch "${marker}"
  local label=""
  label="$(softwareupdate -l 2>/dev/null | sed -n 's/^[[:space:]]*\*[[:space:]]*Label:[[:space:]]*//p' | grep -i 'Command Line Tools' | tail -n1 || true)"
  if [[ -z "${label}" ]]; then
    label="$(softwareupdate -l 2>/dev/null | grep -i 'Command Line Tools' | tail -n1 | sed -E 's/^[[:space:]]*\*?[[:space:]]*//' || true)"
  fi
  if [[ -n "${label}" ]]; then
    sudo softwareupdate -i "${label}" --agree-to-license || {
      sudo rm -f "${marker}"
      die "softwareupdate failed for ${label}"
    }
    sudo rm -f "${marker}"
    xcode-select -p >/dev/null 2>&1 || die "Xcode CLT still missing after softwareupdate"
    return 0
  fi
  sudo rm -f "${marker}"
  log "softwareupdate catalog had no CLT label; opening xcode-select --install and polling"
  xcode-select --install >/dev/null 2>&1 || true
  local i=0
  while ! xcode-select -p >/dev/null 2>&1; do
    i=$((i + 1))
    if (( i > 180 )); then
      die "Xcode CLT still missing after 30m poll (GUI dialog may be stuck)"
    fi
    sleep 10
  done
}

# nix-darwin manages the Brewfile but never installs Homebrew: with brew missing
# its activation just prints "Homebrew is not installed, skipping" and continues,
# so a fresh Mac would silently come up with no casks at all.
ensure_homebrew() {
  if [[ -x /opt/homebrew/bin/brew ]]; then
    log "Homebrew already installed"
    return 0
  fi
  log "installing Homebrew (NONINTERACTIVE=1; no RETURN prompt)"
  if NONINTERACTIVE=1 /bin/bash -c \
      "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"; then
    log "Homebrew installed"
  else
    log "WARNING: Homebrew install failed; every brew/cask outcome (incl. karabiner-elements) skips this run"
  fi
}

darwin_rebuild_switch() {
  local host="$1"
  source_nix_daemon
  cd "${REPO_DIR}"
  log "darwin-rebuild switch --flake .#${host} (Kanata daemon OFF by flake default)"
  # Never pass an override that enables Kanata from this script. The nix run
  # form is what upstream documents for a machine with no darwin-rebuild yet.
  if command -v darwin-rebuild >/dev/null 2>&1; then
    sudo darwin-rebuild switch --flake ".#${host}"
  else
    sudo "$(command -v nix)" run nix-darwin/master#darwin-rebuild -- switch --flake ".#${host}"
  fi
}

run_post_switch() {
  export PATH="/run/current-system/sw/bin:/usr/local/bin:${HOME}/.local/bin:${PATH}"
  command -v sunghyun >/dev/null 2>&1 \
    || die "sunghyun missing after darwin-rebuild switch (flake package should have shipped it)"
  if detect_headless; then
    export SUNGHYUN_HEADLESS=1
    log "headless/no Aqua session: post-switch --headless (GUI surfaces skip; not failures)"
    sunghyun --headless post-switch || true
    sunghyun --headless verify || true
    return 0
  fi
  log "post-switch (opens Settings panes for one-time grants and polls; no prompts)"
  sunghyun post-switch || true
  sunghyun verify || true
}

enable_kanata_safe() {
  if [[ "${SUNGHYUN_KEYBOARD_ENGINE:-karabiner}" != "kanata" ]]; then
    log "keyboard engine: Karabiner-Elements (default); kanata stays disabled"
    return 0
  fi
  export PATH="/run/current-system/sw/bin:/usr/local/bin:${HOME}/.local/bin:${PATH}"
  if detect_headless; then
    log "headless: skipping kanata enable --safe (no typing proof)"
    return 0
  fi
  if ! command -v sunghyun >/dev/null 2>&1; then
    log "sunghyun missing; cannot safe-enable kanata"
    return 0
  fi
  log "kanata: safe enable (passthrough proof + rollback watchdog; opens Input Monitoring pane if needed)"
  if sunghyun kanata enable --safe; then
    log "kanata: enabled"
    sunghyun kanata status || true
  else
    log "kanata: safe enable failed; left disabled (install continues)"
    sunghyun kanata disable || true
  fi
}

main_darwin() {
  sudo_keepalive_start
  ensure_xcode_clt
  ensure_nix
  ensure_homebrew
  ensure_repo

  local host
  host="$(resolve_flake_host)"
  log "flake host: ${host}"
  darwin_rebuild_switch "${host}"
  run_post_switch
  enable_kanata_safe

  log "one-shot complete"
}

linux_home_config() {
  case "$(uname -m)" in
    aarch64 | arm64) printf 'sc@aarch64-linux\n' ;;
    x86_64 | amd64) printf 'sc@x86_64-linux\n' ;;
    *) die "unsupported Linux architecture: $(uname -m)" ;;
  esac
}

main_linux() {
  command -v git >/dev/null 2>&1 || die "git is required on Linux (apt install git)"
  # The Determinate installer needs root; without this it would prompt for a
  # password in the middle of the pipe.
  sudo_keepalive_start
  ensure_nix
  ensure_repo

  export SUNGHYUN_HEADLESS=1
  cd "${REPO_DIR}"
  local hm out
  hm="$(linux_home_config)"
  log "home-manager switch --flake .#${hm} (portable layer; no GUI surfaces)"
  # Built from THIS flake so the pinned home-manager input is what runs:
  # `nix run home-manager/master` fetches a floating upstream instead.
  out="$(nix build --no-link --print-out-paths \
    ".#homeConfigurations.\"${hm}\".activationPackage")" \
    || die "home-manager activation package build failed for ${hm}"
  HOME_MANAGER_BACKUP_EXT=hm-backup "${out}/activate"
  log "one-shot complete (Linux portable layer)"
}

main() {
  case "$(uname -s)" in
    Darwin) main_darwin ;;
    Linux) main_linux ;;
    *) die "unsupported OS: $(uname -s)" ;;
  esac
}

main "$@"
