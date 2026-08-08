# sunghyun.nix

Outcome-driven machine setup: a **nix-darwin** flake owns everything declarative on the Mac, a portable **Home Manager** layer covers Linux hosts, plus a small Rust CLI (`sunghyun`) for what Nix genuinely cannot do. The spec is [OUTCOMES.md](OUTCOMES.md), not any particular tool.

**One-shot is the only setup UX.** Paste one command; the script installs Nix, applies the flake, and surfaces macOS's own one-time permission prompts (opens the exact Settings pane, polls, skips gracefully on timeout). Never a multi-step “install Nix, then rebuild, then post-switch” primary path.

Framework [anaclumos/nix](https://github.com/anaclumos/nix) stays NixOS/keyd-only. Do not merge this into that flake.

History note (2026-08-08): this repo replaces the private `anaclumos/sunghyun-os` (deleted per owner decision; recreated public from an audited tree). The binary was renamed `sunghyun-os` → `sunghyun`.

## Setup (macOS or Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/anaclumos/sunghyun.nix/main/install.sh | bash
```

macOS: Terminal once for the sudo password. The only other human surface is macOS's own one-time prompts (TCC toggles, dext approval) in windows the script opens; Apple ID is assumed from Setup Assistant (signed out ⇒ mas apps skip and converge later).

Linux (non-NixOS, e.g. Ubuntu servers): same one-liner; it detects Linux and applies the portable Home Manager layer (`.#sc@linux`) with Determinate Nix. No GUI steps; headless-safe.

What it does (macOS):

- Xcode CLT (noninteractive when the catalog allows)
- Determinate Nix (`install --no-confirm`)
- Clone/update this repo (`~/Developer/sunghyun.nix`), the only repo the run needs
- `darwin-rebuild switch --flake .#auracomputer` (or `SUNGHYUN_HOST` / matching LocalHostName); the flake builds and ships the `sunghyun` binary
- `sunghyun post-switch` (opens Settings panes for one-time grants, polls, skips on timeout)

**Keyboard engine: Karabiner-Elements** (declarative `karabiner.json` via Home Manager; see the engine evaluation in [OUTCOMES.md](OUTCOMES.md)). **Kanata is the opt-in alternative** (`SUNGHYUN_KEYBOARD_ENGINE=kanata`): the flake defaults its LaunchDaemon **off** so a bare rebuild cannot brick typing, and it is enabled only via `sunghyun kanata enable --safe` (kanata ≥ 1.12.0 → VirtualHID daemon → passthrough proof → full config proof → LaunchDaemon; automatic `kanata disable` rollback on failure). Emergency: `sunghyun kanata disable`. Both paths keep the `launchctl disable system/com.anaclumos.kanata` override consistent: disable records it, safe-enable clears it, so a bare plist rename can never re-arm the daemon on boot.

Headless VM: same one-liner with `SUNGHYUN_HEADLESS=1` (GUI surfaces skip; skips are not failures).

Module map: [docs/nix-darwin.md](docs/nix-darwin.md).

### Module map (short)

| Path | Role |
|---|---|
| `install.sh` | **Only** setup entry (darwin + linux) |
| `flake.nix` | `darwinConfigurations.auracomputer`, `homeConfigurations."sc@linux"` |
| `nix/darwin/hosts/auracomputer.nix` | Host naming |
| `nix/darwin/modules/base.nix` | primaryUser, stateVersion, Touch ID sudo |
| `nix/darwin/modules/homebrew.nix` | brews/casks + non-fatal mas activation |
| `nix/darwin/modules/kanata.nix` | Root LaunchDaemon (**default off**; opt-in engine) |
| `nix/darwin/modules/defaults.nix` | Pinned `system.defaults` |
| `nix/darwin/modules/home.nix` | HM (darwin): karabiner.json, `~/.config/sunghyun/*` |
| `nix/home/portable.nix` | HM (portable): hushlogin, vendored zsh dotfiles (darwin + linux) |
| `nix/darwin/modules/sunghyun.nix` | Flake-built CLI + stable `/usr/local/bin` copy (TCC path) |

## Dotfiles wiring

The zsh configuration is **content in this repo** (`assets/dotfiles/zsh/`). Home Manager links `~/.zshenv` / `~/.zshrc` / `~/.zprofile` / `~/.zlogin` and `~/.config/zsh/{lib,rc,bin}` from the Nix store, so the files are read-only and no vendor installer can append to them behind the flake's back. `~/.zshenv` sets `ZSH_CONFIG_HOME=~/.config/zsh`; the rc files source `lib/` and `rc/` from there.

Edit the dotfiles in `assets/dotfiles/zsh/` and `darwin-rebuild switch` (`z` opens this repo). There is no second repo and no working-copy dependency: the machine converges with nothing else cloned. `programs.zsh` stays off, since it would generate rc content and take ownership.

## CLI (runtime)

```bash
sunghyun open ghostty
sunghyun open-default-browser
sunghyun input-source ABC
sunghyun tile left
sunghyun launcher --query Ghostty
sunghyun spotlight restore
sunghyun verify
sunghyun kanata status
sunghyun kanata enable --safe   # install.sh path; proof + rollback
sunghyun kanata disable         # emergency
sunghyun post-switch            # used by install.sh; not a human follow-up step
```

Default Mac launcher is **Apple Spotlight on Cmd-Space**. Hyper+J opens the **OS default browser**.

### Spotlight: `terminal` → Ghostty

`post-switch` installs `~/Applications/terminal.app`, a thin wrapper that runs `open -b com.mitchellh.ghostty`.

### Spotlight clipboard history

On macOS Tahoe+, Clipboard History is system Spotlight: ⌘Space then ⌘4 (Apple ships no global hotkey). ⌘⇧V is bound in `karabiner.json` to send exactly that sequence as virtual HID key events — no shell hop, no TCC dependency. `sunghyun spotlight clipboard` is the manual CLI equivalent (posts CGEvents; needs the binary's Accessibility grant).

## Permissions (GUI Mac)

Policy: the system opens the exact Settings pane (or lets macOS prompt) and polls; the owner clicks the toggle; timeouts skip gracefully and converge on the next switch.

| Capability | Permission |
|---|---|
| Karabiner-Elements remap | macOS's own prompts on first launch (grabber); DriverKit dext approval |
| Kanata (opt-in engine) | Input Monitoring; Karabiner-DriverKit VirtualHIDDevice **v6.2.0**; kanata ≥ 1.12.0; enable only via `kanata enable --safe` |
| `tile` / clipboard paste | Accessibility for `/usr/local/bin/sunghyun` itself (pane opened + polled by post-switch) |
| `mas` apps | Apple ID from Setup Assistant; signed out ⇒ graceful skip |

TCC notes (2026-08-08): grants attach to the binary's code identity, so every rebuild that changes the binary drops the Accessibility grant until post-switch re-converges. Tiling runs on the native AX API inside the binary (not osascript), so the direct grant on `/usr/local/bin/sunghyun` is sufficient even when Karabiner's `karabiner_console_user_server` spawns it. `verify` probes the binary's own grant with TCC responsibility disclaimed; trust inherited from a terminal never false-greens it.

Pinned driver: [Karabiner-DriverKit-VirtualHIDDevice v6.2.0](https://github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice/releases/tag/v6.2.0).

## Keyboard notes

- Caps tap → tile maximize; Caps hold → Hyper.
- Hyper arrows / 1-4 / Enter → tiling; Hyper+W → right three quarters; Hyper+J → `open-default-browser`.
- L⌘ tap → ABC; R⌘ tap → 2-Set Korean; hold → normal ⌘ chords (⌘C/V).
- ⌘⇧V → Spotlight Clipboard History (virtual ⌘Space, ⌘4).
- Delivered by Karabiner-Elements complex modifications (`assets/karabiner.json`, Home Manager-managed).
- Kanata brick class: grab without VirtualHID output kills all typing including OSK → use `kanata enable --safe` / `kanata disable` only.

## Verify

`sunghyun verify` reports `ok` / `skipped` / `failed`. Exit `0` when there are no hard failures (headless skips count as skipped). Install runs verify itself; it is not a separate human step.

## License

MIT
