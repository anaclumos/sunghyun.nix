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
# Cursor Agent (`cursor-agent`) ships with the switch: macOS through the
# official `cursor-cli` Homebrew cask declared in the flake, Linux through
# nixpkgs in the portable layer. Installing it is all the entry point can do —
# it cannot sign in, because `agent login` is a browser OAuth flow and the only
# alternative is a CURSOR_API_KEY, neither of which a script may invent.
#
# Linux (non-NixOS, e.g. Ubuntu servers, screenless devices): Determinate Nix +
# standalone Home Manager (`.#sc@x86_64-linux` / `.#sc@aarch64-linux`, portable
# layer only). No GUI steps; headless-safe.
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
# Fallback flake host for a machine this repo has no named config for. It must
# stay `default`: `.#default` is the only darwin config that sets no
# ComputerName/LocalHostName, so an unknown Mac keeps the identity Setup
# Assistant gave it. Pointing this at a named host renames the machine (a VM
# once collided with the real Mac on the LAN and became auracomputer-2.local).
DEFAULT_HOST="${SUNGHYUN_HOST:-default}"
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

# A cached sudo ticket cannot survive this run. Homebrew resets it on every
# brew invocation, unconditionally and with no opt-out:
#   Library/Homebrew/brew.sh
#     # Reset sudo timestamp to avoid running unauthorized sudo commands
#     "${SUDO}" --reset-timestamp
# and brew runs here twice -- once from ensure_homebrew, once from the Brewfile
# activation inside darwin-rebuild. A keep-alive loop cannot recover, because
# re-minting a ticket needs the password again. So the single authorization the
# owner gives is held in a sudoers drop-in for the length of the run and dropped
# on the way out. A leaked grant from a SIGKILLed run is reclaimed by the next
# run, which reinstalls and then releases the same path.
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
  # Already root (common on a freshly provisioned Linux server): nothing to
  # authenticate, nothing to keep alive, and no drop-in to leave behind.
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

# nix-darwin manages the Brewfile but never installs Homebrew: when brew is
# missing its activation only prints "error: Homebrew is not installed,
# skipping..." and continues, so a fresh Mac would silently come up without any
# cask -- including karabiner-elements, the primary keyboard engine.
ensure_homebrew() {
  if [[ -x /opt/homebrew/bin/brew ]]; then
    log "Homebrew already installed"
    return 0
  fi
  log "installing Homebrew (NONINTERACTIVE=1; no RETURN prompt)"
  # Official installer; NONINTERACTIVE=1 is its documented unattended mode.
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
  # System activation must run as root; unprivileged it aborts with
  # "system activation must now be run as root". The bootstrap form is the one
  # upstream documents for a machine that has no darwin-rebuild in PATH yet.
  # Never pass an override that enables Kanata from this script.
  if command -v darwin-rebuild >/dev/null 2>&1; then
    sudo darwin-rebuild switch --flake ".#${host}"
  else
    sudo "$(command -v nix)" run nix-darwin/master#darwin-rebuild -- switch --flake ".#${host}"
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
  ensure_homebrew
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
# There is exactly one output per Linux system, because a Home Manager
# configuration is built for a fixed platform: the old single `sc@linux`
# pinned x86_64-linux and could not activate on an aarch64 box at all.
linux_home_config() {
  case "$(uname -m)" in
    aarch64 | arm64) printf 'sc@aarch64-linux\n' ;;
    x86_64 | amd64) printf 'sc@x86_64-linux\n' ;;
    *) die "unsupported Linux architecture: $(uname -m)" ;;
  esac
}

main_linux() {
  command -v git >/dev/null 2>&1 || die "git is required on Linux (apt install git)"
  # The Determinate installer needs root. Without this the install prompts for
  # a password in the middle of the pipe, which is the one thing the one-shot
  # entry point promises never to do.
  sudo_keepalive_start
  ensure_nix
  ensure_repo
  ensure_configs

  export SUNGHYUN_HEADLESS=1
  cd "${REPO_DIR}"
  local hm out
  hm="$(linux_home_config)"
  log "home-manager switch --flake .#${hm} (portable layer; no GUI surfaces)"
  # Build the activation package from THIS flake so the pinned home-manager
  # input is what runs. `nix run home-manager/master` fetched a floating
  # upstream instead, so the activated generation could disagree with
  # flake.lock. `-b hm-backup` is just this env var under the hood.
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
