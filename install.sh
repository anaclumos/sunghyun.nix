#!/usr/bin/env bash
# One-shot machine setup for sunghyun.nix.
#
#   curl -fsSL https://raw.githubusercontent.com/anaclumos/sunghyun.nix/main/install.sh | bash
#
# macOS: Xcode CLT + Determinate Nix + darwin-rebuild switch + `sunghyun
# post-switch`. Idempotent / retry-safe. Requires a Terminal once for the sudo
# password, then continues unattended. The only other human surface is macOS's
# own one-time prompts (TCC toggles, dext approval): the script opens the
# exact Settings pane and polls; timeouts skip gracefully and converge on the
# next switch. Apple ID: assumed from Setup Assistant; if signed out, mas apps
# skip.
#
# Linux (non-NixOS, e.g. Ubuntu servers): Determinate Nix + standalone Home
# Manager (`.#sc@linux`, portable layer only). No GUI steps; headless-safe.
#
# Keyboard engine (macOS): Karabiner-Elements (declarative karabiner.json via
# Home Manager; cask via nix-darwin homebrew). Kanata is the opt-in
# alternative (SUNGHYUN_KEYBOARD_ENGINE=kanata); its LaunchDaemon is OFF by
# flake default and only ever enabled via `sunghyun kanata enable --safe`
# (VirtualHID up → passthrough proof → full config proof → LaunchDaemon;
# automatic rollback/disable on any failed proof).
#
# Headless / no WindowServer: set SUNGHYUN_HEADLESS=1 (or auto-detect) to
# skip GUI surfaces without hanging forever.
set -euo pipefail

REPO_URL="${SUNGHYUN_REPO_URL:-https://github.com/anaclumos/sunghyun.nix.git}"
REPO_DIR="${SUNGHYUN_DIR:-$HOME/Developer/sunghyun.nix}"
CONFIGS_URL="${SUNGHYUN_CONFIGS_URL:-https://github.com/anaclumos/configs.git}"
CONFIGS_DIR="${SUNGHYUN_CONFIGS_DIR:-$HOME/Developer/configs}"
DEFAULT_HOST="${SUNGHYUN_HOST:-auracomputer}"
export PATH="${HOME}/.local/bin:/nix/var/nix/profiles/default/bin:/run/current-system/sw/bin:/opt/homebrew/bin:/usr/local/bin:${PATH}"

log() { printf 'sunghyun-install: %s\n' "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

# The run is unattended after the single sudo password, so git must never stop
# to ask for credentials on a repo it cannot read.
export GIT_TERMINAL_PROMPT=0

# --- sudo keep-alive (single password, then unattended) ---------------------
# The password gate must test the controlling terminal, not stdin: under the
# documented entry point (`curl ... | bash`) stdin is the curl pipe, so `-t 0`
# is false even in Terminal, and sudo reads from /dev/tty anyway.
has_controlling_tty() { { : < /dev/tty; } 2>/dev/null; }

sudo_keepalive_start() {
  if sudo -n true 2>/dev/null; then
    :
  elif has_controlling_tty; then
    log "sudo: enter password once; install continues unattended after this"
    sudo -v < /dev/tty || die "sudo -v failed"
  else
    die "sudo needs a password once but there is no controlling terminal; run this from Terminal"
  fi
  (
    while true; do
      sudo -n true 2>/dev/null || true
      sleep 60
      kill -0 "$$" 2>/dev/null || exit 0
    done
  ) &
  SUDO_KEEPALIVE_PID=$!
  trap 'kill "${SUDO_KEEPALIVE_PID}" 2>/dev/null || true' EXIT INT TERM
}

# --- environment helpers ----------------------------------------------------
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
  # launchctl managername == Aqua ⇒ GUI login session
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

# The portable HM layer symlinks ~/.zsh* into this working copy (single
# canonical dotfiles source; no managed second clone).
#
# anaclumos/configs is private, so on a fresh Mac -- which has no GitHub
# credentials until the owner signs in -- this clone legitimately cannot
# succeed. Treat it like the other unauthenticated surfaces (mas / Apple ID):
# skip, never block, converge on a later run. HM only pins out-of-store
# symlinks into this path, so a missing clone degrades to dangling ~/.zsh*
# instead of failing activation.
ensure_configs() {
  if clone_or_update "${CONFIGS_URL}" "${CONFIGS_DIR}"; then
    return 0
  fi
  log "configs: ${CONFIGS_URL} unavailable (private repo, no credentials yet); skipping, converges on a later run"
  return 0
}

# --- macOS steps --------------------------------------------------------------
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
  # Last resort: GUI installer (still no yes|); poll until present.
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

darwin_rebuild_switch() {
  local host="$1"
  source_nix_daemon
  cd "${REPO_DIR}"
  log "darwin-rebuild switch --flake .#${host} (Kanata daemon OFF by flake default)"
  # Never pass an override that enables Kanata from this script.
  if command -v darwin-rebuild >/dev/null 2>&1; then
    darwin-rebuild switch --flake ".#${host}"
  else
    nix run nix-darwin -- switch --flake ".#${host}"
  fi
}

run_post_switch() {
  # The flake builds and ships `sunghyun` (systemPackages + /usr/local/bin copy).
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
  # Residual TCC/dext surfaces: the CLI opens each Settings pane and polls;
  # the owner clicks the toggle in the opened window. Timeouts skip, never fail.
  log "post-switch (opens Settings panes for one-time grants and polls; no prompts)"
  sunghyun post-switch || true
  sunghyun verify || true
}

enable_kanata_safe() {
  # Kanata is the opt-in alternative engine; Karabiner-Elements is the default
  # (handled declaratively + by post-switch). Only run on explicit opt-in.
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
  ensure_repo
  ensure_configs

  local host
  host="$(resolve_flake_host)"
  log "flake host: ${host}"
  darwin_rebuild_switch "${host}"
  run_post_switch
  enable_kanata_safe

  log "one-shot complete"
}

# --- Linux steps ----------------------------------------------------------------
main_linux() {
  command -v git >/dev/null 2>&1 || die "git is required on Linux (apt install git)"
  ensure_nix
  ensure_repo
  ensure_configs

  export SUNGHYUN_HEADLESS=1
  cd "${REPO_DIR}"
  log "home-manager switch --flake .#sc@linux (portable layer; no GUI surfaces)"
  nix run home-manager/master -- switch --flake ".#sc@linux" -b hm-backup
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
