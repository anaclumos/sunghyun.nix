# nix-darwin and sunghyun.nix

**One-shot is the only New Mac UX.** Do not document a multi-step “install Nix → rebuild → post-switch” primary path. Do not merge this flake into [anaclumos/nix](https://github.com/anaclumos/nix) (Framework NixOS / keyd only).

## Top-level command

```bash
curl -fsSL https://raw.githubusercontent.com/anaclumos/sunghyun.nix/main/install.sh | bash
```

`install.sh` installs Determinate Nix, clones this repo (the only repo it needs), runs `darwin-rebuild`, and runs `post-switch` (opens Settings panes for one-time grants, polls, skips on timeout). It never asks the human to run a follow-up command. Keyboard engine is Karabiner-Elements; the Kanata LaunchDaemon stays **disabled** unless explicitly opted in via `SUNGHYUN_KEYBOARD_ENGINE=kanata` + `kanata enable --safe`.

On Linux the same script applies the portable Home Manager layer (`.#sc@linux`); no GUI steps, headless-safe.

## Architecture

```
install.sh                                         ← only New Mac entry
  ├─ Xcode CLT, Determinate Nix
  ├─ darwin-rebuild switch --flake .#auracomputer  ← nix-darwin + Home Manager
  │    ├─ homebrew.brews / casks (+ non-fatal mas postActivation)
  │    ├─ karabiner.json via home.file (primary keyboard engine)
  │    ├─ launchd Kanata daemon (default OFF; opt-in engine)
  │    ├─ flake-built sunghyun → /usr/local/bin (TCC path stability)
  │    └─ system.defaults (pinned keys only)
  └─ sunghyun post-switch                          ← open Settings pane + poll; skip on timeout
```

| Layer | Owns |
|---|---|
| **install.sh** | One-shot orchestration (darwin + linux) |
| **nix-darwin flake** | Declarative Mac state |
| **sunghyun CLI** | OS-prompt surfaces + keyboard actions (invoked by install.sh) |
| **anaclumos/nix** | Framework NixOS only |

## Module map

| Module | Role |
|---|---|
| `base.nix` | `system.stateVersion`, `system.primaryUser`, zsh, Determinate-safe `nix.enable = false`, Touch ID sudo |
| `homebrew.nix` | Formulae/casks; `cleanup = "none"`; non-fatal mas via `postActivation` |
| `kanata.nix` | Root LaunchDaemon; `services.sunghyun.kanata.enable` (default **false**) |
| `defaults.nix` | Only verified `system.defaults` keys |
| `home.nix` | Home Manager files (darwin): karabiner.json, keyboard assets |
| `../home/portable.nix` | Home Manager files (portable): `.hushlogin`, zsh dotfiles from `assets/dotfiles/zsh` |
| `sunghyun.nix` | Flake-built CLI + `/usr/local/bin` copy via `extraActivation` |
| `hosts/auracomputer.nix` | Host naming |

## Capability map

| Concern | Who closes it |
|---|---|
| Homebrew | Flake (`brew bundle` on activation) |
| mas apps | Non-fatal `postActivation`; Apple ID from Setup Assistant, else graceful skip |
| Keyboard engine (Karabiner-Elements) | Cask + HM karabiner.json; post-switch launches once so macOS prompts |
| Kanata root daemon | Off by default; opt-in via `kanata enable --safe` only |
| Accessibility (sunghyun) | post-switch opens the Privacy pane and polls; timeout = skip (grant is per code identity; rebuilt binaries re-converge here) |
| DriverKit first approval | post-switch installs pkg when needed, opens Login Items pane, polls |
| Spotlight ⌘Space | post-switch inside install.sh |
| Secrets / Apple ID password | Human dialog only; the system opens the UI and waits |

## What NOT to put in Nix

- TCC grants or any `TCC.db` write
- Interactive “press Enter when done” gates as the primary UX
- Secrets or maps to secret files
- Framework/keyd modules from anaclumos/nix

## Safety

- `homebrew.onActivation.cleanup = "none"`
- Activation never waits on TCC/App Store and never prints “run post-switch next”
- Kanata daemon default off; points at Homebrew `kanata` + HM `~/.config/sunghyun/kanata.kbd` when enabled later
- `sunghyun kanata disable` records a `launchctl disable system/com.anaclumos.kanata` override; safe-enable clears it (a bare plist rename cannot re-arm the daemon on boot)
