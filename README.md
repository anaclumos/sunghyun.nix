# sunghyun.nix

Fully declarative machine setup for macOS and Linux.

- A **nix-darwin** flake owns the Mac. A portable **Home Manager** layer covers Linux hosts, headless included.
- Repo rules (owner 2026-08-08): no comments in nix files, no runtime shell anywhere in the tree. Declarative options only. What genuinely needs code runs inside Hammerspoon as Lua, with private-framework calls as in-process JXA.
- The spec is the [Outcomes](#outcomes) ledger below, not any particular tool.
- Framework [anaclumos/nix](https://github.com/anaclumos/nix) stays NixOS/keyd-only. Do not merge this flake into it.
- History (2026-08-08): this repo replaces the private `anaclumos/sunghyun-os` (deleted per owner decision; recreated public from an audited tree). The declarative rewrite of 2026-08-08 then retired the flake-built `sunghyun` CLI, `install.sh`, the Kanata engine and every converge script.

## Setup

macOS, fresh machine:

```bash
xcode-select --install
curl -fsSL https://install.determinate.systems/nix | sh -s -- install --no-confirm
git clone https://github.com/anaclumos/sunghyun.nix.git ~/Developer/sunghyun.nix
sudo nix run nix-darwin/master#darwin-rebuild -- switch --flake ~/Developer/sunghyun.nix#auracomputer
```

- An unknown Mac uses `.#default`, the only config with all naming fields unset, so it keeps its own identity (row q).
- After the first switch, `sudo darwin-rebuild switch --flake .#auracomputer` or the zsh `build` function (row aq).
- Linux (non-NixOS, e.g. Ubuntu servers): `nix run home-manager -- switch --flake .#sc@x86_64-linux` (or `sc@aarch64-linux`; `sc@linux` aliases x86_64). Headless-safe, no GUI steps.

## Layout

| Path | Role |
|---|---|
| `flake.nix` | `darwinConfigurations.{auracomputer,default}`, per-arch `homeConfigurations`, checks, formatter; inputs [sunghyun-sans](https://github.com/anaclumos/sunghyun-sans) and [tokenmaxxing](https://github.com/anaclumos/tokenmaxxing) |
| `nix/darwin/hosts/` | `auracomputer.nix` names the machine; `default.nix` names nothing |
| `nix/darwin/modules/agents.nix` | The nix half of the layered agent config: Codex system `config.toml` and Claude Code managed settings (row au) |
| `nix/darwin/modules/base.nix` | primaryUser, `nixpkgs.config.allowUnfree`, `nix.enable = false`, Touch ID sudo, fonts |
| `nix/darwin/modules/homebrew.nix` | Taps, brews, casks, `masApps`, `cleanup = "uninstall"` with the karabiner-elements eval assertion |
| `nix/darwin/modules/defaults.nix` | Every `system.defaults` key, including `CustomUserPreferences` for Finder desktop view and KakaoTalk language |
| `nix/darwin/modules/hammerspoon.nix` | Hammerspoon cask + user LaunchAgent |
| `nix/darwin/modules/home.nix` | HM (darwin): generated `~/.hammerspoon/init.lua`, `karabiner.json`, ghostty config |
| `nix/hammerspoon.nix` | Generates init.lua: tile geometry, Hyper chord bindings, dark-mode JXA, default-browser open |
| `nix/home/portable.nix` | HM (darwin + linux): hushlogin, vendored zsh dotfiles, shared `home.packages`, agent-guide links |
| `nix/home/linux.nix` | Linux-only packages and the tokenmaxxing module |
| `nix/home/fonts.nix` | Linux fonts + fontconfig |
| `nix/pkgs/resend-cli.nix` | Flake-local derivation for the Resend CLI |
| `assets/` | karabiner.json, ghostty config, vendored zsh tree |

## Packages

One channel per distribution reality. Installing is the declarative outcome; every sign-in stays with the owner's session.

| Channel | Members |
|---|---|
| Homebrew brews | fnm, gh, mas, mole, pscale, ripgrep, `getsentry/tools/sentry`, stripe-cli, tmux, vercel |
| Homebrew casks | 1password, aside, claude-code, codex, codexbar, cursor, cursor-cli, ghostty, hammerspoon, iina, itsycal, karabiner-elements, keka, linear, macs-fan-control, obsidian, orbstack, slack, tailscale-app |
| `homebrew.masApps` | KakaoTalk, What Watt?, Amphetamine |
| nixpkgs `home.packages` (macOS + Linux) | btop, bun, dotenvx, google-cloud-sdk, inngest, resend (`nix/pkgs/resend-cli.nix`) |
| nixpkgs, Linux only | claude-code, codex, cursor-cli |
| Manual | mint (Mintlify): `bun add --global mint`, lands in `~/.bun/bin`; Xcode from [Apple Beta](https://developer.apple.com/download/) |

- Brew carries vendor self-update channels and macOS-only tools. The coding-CLI casks stay writable so `cursor-agent update` and friends work, which a Nix store copy cannot.
- `stripe-cli` installs `stripe`. The sentry tap formula installs `sentry`; `sentry-cli` is a zsh alias that prints "Use `sentry --help`" because the homebrew-core `sentry-cli` formula is a different tool (owner 2026-08-08).
- inngest has no homebrew-core formula, only a third-party tap, so it comes from nixpkgs; it is SSPL, covered by `nixpkgs.config.allowUnfree` in `base.nix` and in the Linux pkgs imports.
- The published `resend-cli` npm tarball ships a self-contained `dist/cli.cjs`, so the derivation is a node wrapper around it, no npm build.
- mint stays outside Nix: its keytar dependency fails to compile under nixpkgs clang and needs prebuilds, which bun installs. With activation hooks banned, converging it is a one-line manual step.
- dotenvx comes from nixpkgs, never its third-party tap: on macOS 27 Homebrew refuses that formula while `/Applications/Xcode.app` trails the CLT (row as).
- gcloud comes from nixpkgs `google-cloud-sdk`, never the `gcloud-cli` cask: the cask's install step hardcodes a Homebrew python path and dies asking for `gcloud config virtualenv create` ([homebrew-cask#241514](https://github.com/Homebrew/homebrew-cask/issues/241514), hit live 2026-08-08, it failed the whole switch). The nixpkgs package wraps gcloud with its own pinned python, so the extension-module virtualenv dance never happens; extra components go through `google-cloud-sdk.withExtraComponents`, not `gcloud components install`.
- nixpkgs bun guarantees bun on a fresh machine; a curl-installed `~/.bun` wins on PATH and keeps its own upgrade channel.
- On macOS the coding CLIs come from Homebrew only, never also from `home.packages`, so two binaries cannot fight over PATH order.
- Xcode comes from Apple Beta, never the App Store (owner 2026-08-08): the App Store build trails the CLT (26.6 vs 27, row as), and beta downloads sit behind the owner's developer sign-in, so no channel here can carry them. `Xcode-beta.app` is installed live; `mas` cannot uninstall, so removing the old App Store copy stays manual.

## Dotfiles

The zsh configuration is content in this repo (`assets/dotfiles/zsh/`).

- Home Manager links `~/.zshenv` / `~/.zshrc` / `~/.zprofile` / `~/.zlogin` and `~/.config/zsh/{lib,rc,bin}` from the Nix store, so the files are read-only and no vendor installer can append to them behind the flake's back.
- Edit in `assets/dotfiles/zsh/` and switch (`z` opens this repo). No second repo, no working-copy dependency.
- `programs.zsh` stays off. It would generate rc content and take ownership away from the vendored files.

## Keyboard

**Karabiner-Elements** is the engine, declarative `karabiner.json` via Home Manager. **Hammerspoon** is the action host: one long-lived app holds the Accessibility grant and runs the window and system actions.

- Caps tap maximizes the window, Caps hold is Hyper (row a).
- Hyper arrows / 1-4 / Enter tile, Hyper+W right three quarters, Hyper+C center, Hyper+V top-left (row b).
- Hyper+J opens the OS default browser; H/I/K/L/M/N/P/R launch apps; D shows the desktop; E and F open Mission Control (row c).
- L⌘ tap switches to ABC, R⌘ tap to 2-Set Korean; held they stay normal ⌘ chords (row d).
- ⌘⇧V opens Spotlight clipboard history (row e). ⌘Space stays Apple Spotlight (row f).
- Top row fires Apple hardware actions bare and function keys on fn, except F4/F5 (row o). A bare fn tap opens Emoji & Symbols (row u). Hyper+grave toggles light/dark appearance (row t).

Wiring: karabiner.json holds the Hyper variable and either acts natively (`software_function.open_application` for app keys) or emits a real ⌘⌃⌥⇧ chord that `hs.hotkey.bind` handles inside the generated init.lua. No shell hop anywhere on the hot path.

Kanata is retired (2026-08-08): the opt-in daemon was default-off, never enabled on any host, and its enable path was CLI machinery this repo no longer carries. The 2026-08-07 brick incident (root exclusive grab with dead VirtualHID output kills every key) stays the reason Karabiner-Elements is the engine: its failure mode is "no remap", never "no keys" (row l).

## Permissions (GUI Mac)

- Homebrew converges on activation (`brew bundle`). `masApps` rides the same Brewfile and needs a signed-in App Store; this machine is signed in and every declared app is installed, so the lines no-op. A fresh signed-out machine fails those lines; sign in and switch again.
- Two one-time human toggles on a fresh machine, each raised by macOS itself: the Karabiner grabber grants at first launch, and Accessibility for Hammerspoon. Both attach to the app bundle, so they survive every rebuild.
- One one-time root command on a fresh machine bridges Claude Code's managed settings to the nix-managed file (row au): `sudo ln -s /etc/claude-code "/Library/Application Support/ClaudeCode"`.
- Sign-in is never automated (Apple ID, Tailscale, OrbStack, Vercel, PlanetScale, Stripe, gcloud, Sentry, Resend, cursor-agent). Install, then let the owner's session take over.
- No nix-darwin options exist for TCC grants, system-extension approval, or App Store sign-in (checked against the full manual 2026-08-07; `TCC.db` is SIP-protected, PPPC profiles are MDM-only).

## Outcomes

Everything here is a desired result, not a stack (owner reframing 2026-08-07). Ids are stable and never renumbered; superseded mechanisms say so with a date. The 2026-08-08 declarative rewrite retired all converge and verify machinery, so a few rows keep their outcome as live converged state that a fresh machine restores by hand.

### Keyboard

| id | outcome | mechanism |
|---|---|---|
| a | Caps tap = maximize; Caps hold = Hyper | karabiner.json variable-based Hyper; the tap emits ⌘⌃⌥⇧M, which Hammerspoon tiles to maximize |
| b | Hyper tiling: halves, fourths, fullscreen, W right three quarters, C center, V top-left; fractions against the visible frame of the focused window's display | karabiner emits ⌘⌃⌥⇧ chords; `hs.hotkey.bind` runs `tile()` in the generated init.lua; geometry is inlined in `nix/hammerspoon.nix`; `screen:frame()` excludes menu bar and Dock |
| c | Hyper app keys activate a running instance instead of starting a second; a missing app raises no dialog; D shows the desktop; E and F open Mission Control (E added by owner 2026-08-08) | karabiner `software_function.open_application` by bundle identifier (native since KE 15.0.19, installed 16.1.0); Hyper+J resolves the LaunchServices http handler via `hs.urlevent.getDefaultHandler` and focuses it; D emits F13, E and F emit ⌃Up, the system Mission Control chord |
| d | L⌘ tap = ABC IME, R⌘ tap = 2-Set Korean; held = normal ⌘ chords | karabiner `to_if_alone` + lazy modifiers firing the system input-source shortcut (^Space, symbolic hot key 60) as a virtual HID chord, gated by `input_source_unless`; per-manipulator 500 ms timeout. Not `TISSelectInputSource`: out-of-process calls update the menu extra without switching the focused app's input context on macOS 26/27 |
| e | ⌘⇧V = Spotlight clipboard history | karabiner sends virtual ⌘Space then ⌘4 as HID events, no shell hop and no TCC |
| o | Top row: bare = Apple hardware actions, fn = function keys, except F4 (bare plain F4, fn Spotlight) and F5 (bare Control-M, fn dictation) | `NSGlobalDomain."com.apple.keyboard.fnState" = false` as the base state plus two karabiner rules for f4/f5, guarded by `variable_unless`. IOHIDSystem reads the preference at login only; the in-session converge is retired (2026-08-08), so a changed value applies at next login |
| t | Hyper+grave toggles light/dark, alternating every press | Hammerspoon runs in-process JXA over SkyLight (`SLSGetAppearanceThemeLegacy` / `SLSSetAppearanceThemeNotifying`), no TCC gate. The Apple Events route would need a consent prompt and a privacy row |
| w | ⌘⇧Space belongs to 1Password Quick Access and nothing else | Hammerspoon, on load, read-modify-writes symbolic hot key 263 (`screenshots.ask-siri-active-window`, key 49, modifiers 1179648) to `enabled = false` via `NSUserDefaults` suite `com.apple.symbolichotkeys`, then runs `activateSettings -u` so the HotKey center drops it; 1Password keeps its native Quick Access chord. Never `CustomUserPreferences` for `AppleSymbolicHotKeys`: nix-darwin's `defaults write` replaces the whole dict and would wipe every other system shortcut |
| u | Bare fn tap opens Emoji & Symbols | `system.defaults.hitoolbox.AppleFnUsageType`; HIToolbox reads it at process start, so a running session converges at next login |
| l | Hard invariant: the keyboard never bricks | Karabiner-Elements' failure mode is "no remap", not "no keys". Kanata, whose root exclusive grab bricked typing on 2026-08-07, is fully retired (2026-08-08): daemon, kbd files and enable machinery deleted. The eval assertion in `homebrew.nix` keeps `karabiner-elements` in the cask list because `cleanup = "uninstall"` would otherwise run its uninstall script and purge the shared DriverKit VirtualHIDDevice files |

### System

| id | outcome | mechanism |
|---|---|---|
| f | ⌘Space stays Apple Spotlight; typing "terminal" launches Ghostty | Converged live; the `~/Applications/terminal.app` wrapper persists. Its installer machinery is retired (2026-08-08) |
| g | Menu bar shows no date; Time Machine extra hidden | `menuExtraClock.ShowDate = 2` and `CustomUserPreferences` systemuiserver keys |
| h | `~/.hushlogin` present | Home Manager `home.file` |
| i | Declared packages and apps present | `homebrew.{brews,casks,masApps}` plus nixpkgs (see [Packages](#packages)) |
| j | zsh, dotfiles and runtimes configured with no second repo to clone | vendored zsh tree, HM store links, `programs.zsh` off (see [Dotfiles](#dotfiles)) |
| k | Fresh-Mac bootstrap is short and unattended after one sudo | Superseded 2026-08-08: `install.sh` is deleted; setup is the four commands under [Setup](#setup). macOS still raises its own one-time dialogs (two "administer your computer" prompts on a first run, measured 2026-08-08) |
| m | Headless targets are first-class | The Linux homeConfigurations carry no GUI surface at all |
| n | Everything idempotent | `darwin-rebuild switch` is the only converge path; `nix flake check` evaluates both darwin systems. The `sunghyun verify` outcome checker is retired (2026-08-08) |
| p | cursor-agent present on macOS and screenless Linux | macOS: `cursor-cli` cask, writable for self-update. Linux: nixpkgs `cursor-cli` |
| v | codex and claude CLIs present on macOS and screenless Linux; interactive zsh ships `cc` = `claude --dangerously-skip-permissions` | macOS: `codex` and `claude-code` casks. Linux: nixpkgs. `cc` is a zsh function in `assets/dotfiles/zsh/rc/aliases.zsh` |
| q | A machine keeps its own identity; only the config that names a machine renames it | `hosts/auracomputer.nix` is the sole config setting naming fields; every other Mac activates `.#default`, whose naming stays unset. Incident behind the rule: a VM that activated the named host came up as `auracomputer-2.local` |
| r | Mac App Store apps converge | `homebrew.masApps` (Brewfile `mas` lines). Supersedes the 2026-08-08 convergence LaunchDaemon, retired the same day in the declarative rewrite. Needs a signed-in App Store; on this machine all three apps are installed so the lines no-op. `mas` cannot uninstall, so removal stays manual. Xcode left this channel for Apple Beta (owner 2026-08-08) |
| s | A VM never wedges on the App Store | Superseded 2026-08-08: the `sunghyun virt` detector is retired with the CLI. A signed-out guest now fails the `mas` Brewfile lines instead of skipping them; the named-host rule (row q) keeps VMs on `.#default` |
| v | Sunghyun Sans present, every family, macOS and Linux | flake input pinned in `flake.lock`; macOS `fonts.packages`, Linux `nix/home/fonts.nix` |
| aa | Dia is gone | Cask dropped 2026-08-08; live uninstall done then |
| ab | Aside is the system default browser, so Hyper+J opens it | Set once live 2026-08-08 (macOS raises its own confirmation panel for this change, no way around it). The setter machinery is retired; Hyper+J follows whatever LaunchServices reports, so the binding stays correct even if the owner changes browsers |
| ac | Dock holds Finder, Downloads and Trash, nothing else | `dock.persistent-apps = [ ]`, `persistent-others` Downloads, `show-recents = false`. Empty list, not null: nix-darwin drops null options, so null is "unmanaged" |
| ad | Desktop shows hard disks, item info under each icon, labels on the right | `finder.ShowHardDrivesOnDesktop` plus `CustomUserPreferences` writing the full `DesktopViewSettings.IconViewSettings` dictionary, values mirroring the converged live state (icon 64, grid 54, text 12). Supersedes the PlistBuddy merge script (retired 2026-08-08): the write replaces the whole dictionary, so the dictionary is declared whole |
| ae | Celsius, metric | `AppleTemperatureUnit`, `AppleMeasurementUnits`, `AppleMetricUnits`, all three because macOS reads all three |
| af | KakaoTalk runs in Korean | `CustomUserPreferences."com.kakao.KakaoTalkMac".AppleLanguages = [ "ko" ]`, the Language & Region per-app mechanism. Best effort: the sandboxed container can shadow the outside domain; the language is already converged live on this machine |
| ag | Tailscale installed; MagicDNS works once signed in | `tailscale-app` cask, the Standalone GUI variant whose Network Extension owns DNS natively |
| ah | Finder windows show path bar and status bar | `finder.ShowPathbar`, `finder.ShowStatusBar` |
| ai | Desktop icons snap to grid | `arrangeBy = "grid"` inside the row ad dictionary |
| aj | tokenmaxxing (`xx`) present on macOS and Linux | flake input; darwin module with overlay, HM module on Linux with the input's package set explicitly |
| ak | Aside installed | `aside` cask, official homebrew/cask token |
| am | Anything Homebrew-managed that this repo does not declare is absent; deleting a definition uninstalls it on the next switch | `onActivation.cleanup = "uninstall"`. Not `"zap"`: zap needs Full Disk Access that sudo activation does not hold. The karabiner-elements assertion (row l) guards the one dangerous deletion. Exception: `mas` apps converge on install only |
| an | Claude Code and Codex sessions start from the canonical agent guide | HM links `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md` to the repo root `AGENTS.md`. Both tools concatenate global then project files, so per-repo guides win conflicts |
| ao | btop present, macOS and Linux | nixpkgs via shared `home.packages` |
| ap | Macs Fan Control installed | `macs-fan-control` cask |
| aq | `build` upgrades the machine the Nix way, never topgrade | zsh `build`: `nix flake update`, then `darwin-rebuild switch` (or the Linux HM activate), then a pathspec-only `flake.lock` commit and push. Brew packages move via `onActivation.{autoUpdate,upgrade}` |
| ar | Menu bar auto-hide is Never, desktop and fullscreen | `_HIHideMenuBar = false` plus `CustomUserPreferences` `AppleMenuBarVisibleInFullscreen` and controlcenter `AutoHideMenuBarOption = 3`; typed `controlcenter` writes ByHost only and cannot reach that domain |
| as | OrbStack, dotenvx, Vercel, pscale, Stripe CLIs present | Casks/brews where the vendor channel works: `orbstack`, `vercel`, `pscale`, `stripe-cli`. dotenvx from nixpkgs: its only brew source is a third-party tap that macOS 27 Homebrew refuses while Xcode trails the CLT |
| at | gcloud, sentry, inngest, mint, resend CLIs present | nixpkgs `google-cloud-sdk` (the `gcloud-cli` cask is broken, see [Packages](#packages)); tap formula `getsentry/tools/sentry` with the `sentry-cli` redirect alias; nixpkgs `bun` and `inngest`; `resend` from `nix/pkgs/resend-cli.nix`. mint is a manual `bun add --global mint` (installed live 2026-08-08): keytar needs prebuilds nixpkgs clang cannot build, and activation hooks are banned, so no machinery converges it |
| au | Agent settings are layered: nix owns the baseline, each tool keeps writing its own on-the-fly edits, in separate files (owner 2026-08-08, reviving the settings baselines from the retired anaclumos/configs instructions pages) | `nix/darwin/modules/agents.nix` via `environment.etc`. Codex: `/etc/codex/config.toml` is the system layer codex merges below the user's `~/.codex/config.toml` and never writes; trust entries, plugin toggles and model picks keep landing in the user file and win per key (verified against the rust-v0.147.0 loader source). Claude Code: `/etc/claude-code/managed-settings.json` carries the managed baseline, bridged on macOS by the one-time symlink under [Permissions](#permissions-gui-mac) since Claude Code reads managed settings from `/Library/Application Support/ClaudeCode`; the tool only ever writes `~/.claude/settings.json`. Managed keys win conflicts, and `model` stays switchable in-session because a managed model is a startup default only. Linux homes carry neither `/etc` layer, Home Manager has no root there |

## Design notes

Choices the code cannot explain by itself.

| Where | Why |
|---|---|
| `base.nix` | `nix.enable = false` because Determinate Nix owns the daemon; nix-darwin managing it would fight the installer over `nix.conf` |
| `base.nix` | `sudo_local` sets `reattach = true` so Touch ID works inside tmux |
| `homebrew.nix` | `HOMEBREW_NO_ASK=1` and `HOMEBREW_NO_ENV_HINTS=1` because Homebrew 6.x prompts y/n by default, which would stall root activation |
| `homebrew.nix` | karabiner-elements is a cask, never `services.karabiner-elements`: that module is broken for KE 15+ ([nix-darwin#1041](https://github.com/nix-darwin/nix-darwin/issues/1041)). Never `brew uninstall --cask karabiner-elements` by hand either |
| `nix/darwin/modules/hammerspoon.nix` | `KeepAlive.SuccessfulExit = false` relaunches a crash and respects a deliberate quit |
| `nix/darwin/modules/home.nix` | karabiner.json ships read-only via `home.file.source` and stays the sole source of truth; Karabiner-Elements live-reloads it and refuses GUI edits against it |
| `nix/hammerspoon.nix` | The dark-toggle JXA sits in a `[=[ ]=]` Lua long string because the ObjC signature arrays contain `]]`, which terminates a plain `[[` string early |
| `nix/home/portable.nix` | `manual.manpages.enable = false`: Home Manager's manual build emits the "options.json references a store path without a proper context" eval warning ([home-manager#7935](https://github.com/nix-community/home-manager/issues/7935), open); the manpage is the only thing in this tree that instantiates it. Re-enable once [#8942](https://github.com/nix-community/home-manager/pull/8942) lands |
| `base.nix` | `system.tools.darwin-uninstaller.enable = false`: the uninstaller embeds its own doc-bearing darwin system, the other carrier of the same options.json warning, and this repo bootstraps by cloning, so an uninstaller script in PATH earns nothing |

## License

MIT
