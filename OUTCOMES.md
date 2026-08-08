# OUTCOMES

Owner reframing (2026-08-07): everything here is a desired result, not a
stack. Every component (Kanata included) is a candidate implementation chosen
on evidence, Nix-first: native macOS/nix-darwin option, then battle-tested
tools configured declaratively, then custom code. Verification asserts
outcomes so the implementation can be swapped without rewriting this spec.

Migration landed (2026-08-08): this is the canonical repo
`anaclumos/sunghyun.nix` (public; replaces the deleted private
`anaclumos/sunghyun-os`; binary renamed to `sunghyun`). Linux devices
including screenless/headless ones are set up via the standalone
`homeConfigurations."sc@x86_64-linux"` / `"sc@aarch64-linux"` outputs (portable
layer only; `"sc@linux"` remains as an x86_64 alias, and `install.sh` picks by
`uname -m`, because a Home Manager configuration is built for one fixed
platform and the old single output could not activate on ARM). Module hygiene
constraint stands: the user-layer Home Manager config
(`nix/home/portable.nix`: dotfiles, hushlogin, future zsh/git/runtimes) must
never import darwin-only options; darwin-specific system state
(system.defaults, launchd, homebrew, keyboard engine) stays in
`nix/darwin/modules/`. GUI-dependent surfaces must keep skipping gracefully
when absent (headless Linux is a first-class target), and shared Rust
CLI/script code paths avoid hardcoded darwin assumptions where a cheap OS
check keeps them portable.

## Keyboard

| # | Outcome | Implementation (today) | Verify predicate |
|---|---------|------------------------|------------------|
| a | Caps tap = maximize window; Caps hold = Hyper | Karabiner-Elements complex modification (`assets/karabiner.json`, Home Manager file, variable-based Hyper) | karabiner.json contains the caps manipulator; `sunghyun tile maximize` works (verify `check_tiles`) |
| b | Hyper + arrows/1-4/Enter = tiling (left/right/top/bottom, fourths, fullscreen); Hyper+W right three quarters, flush to the right edge, left quarter empty; Hyper+C center, Hyper+V top-left. Every fraction is taken against the visible frame (menu bar and Dock excluded) of the display holding the focused window, so a repeated press is a no-op | same | karabiner.json manipulators present; tile actions pass verify |
| c | Hyper+J = open system default browser (H/I/K/L/M/N/P/R = mail/IINA/music/calendar/kakaotalk/slack/preview/linear; D = desktop; F = Mission Control). Owner 2026-08-08: I moved from Linear to IINA, and P and R are new. Each one activates a running instance instead of starting a second, and a missing app fails silently rather than raising a dialog on every press | same, `shell_command → /usr/local/bin/sunghyun open <key>`, resolved through the one generic `open` subcommand and its `[apps.*]` table (`open -b <bundle id>`, which activates rather than duplicates) | manipulators present; `sunghyun open-default-browser` resolves; every bound app key resolves (verify `check_apps`, `check_keyboard_engine`) |
| d | Left ⌘ tap = ABC IME, Right ⌘ tap = 2-Set Korean; held = normal ⌘ chords (⌘C/⌘V must never break) | Karabiner `to_if_alone` + lazy `left_command`/`right_command` | manipulators present; `sunghyun input-source` works (verify `check_ime_mapping`) |
| e | ⌘⇧V = Spotlight clipboard history | Karabiner manipulator sends virtual ⌘Space then ⌘4 as HID key events (no shell hop, no TCC; macOS 27 gates synthetic keystrokes from spawned processes beyond Accessibility) | manipulator present; pasteboard history enabled (verify `check_spotlight_clipboard`) |
| o | Top row, built-in Apple keyboard (owner 2026-08-08). **F1/F2/F3 and F6 through F12**: bare fires the Apple hardware action (brightness down/up, Mission Control, Do Not Disturb, previous/play-pause/next, mute, volume down/up), fn sends the plain function key. **F4**: bare sends a plain F4, fn+F4 opens Spotlight. **F5**: bare sends Control-M, fn+F5 starts dictation; a plain F5 is deliberately unreachable. A non-Apple top row that emits its own consumer usages instead of f1-f12 cannot be expressed this way | nix-darwin `system.defaults.NSGlobalDomain."com.apple.keyboard.fnState" = false`. Apple's media base plus Karabiner 16.1.0's built-in `fn_function_keys` defaults (which already carry the modern M-series `spotlight`/`dictation`/`do_not_disturb` usages) cover ten of the twelve keys with no rule at all. Only f4 and f5 get a rule, two manipulators each, bare and fn, so nothing can double-fire; both are guarded by `variable_unless system.use_fkeys_as_standard_function_keys` so a flipped base state drops the exception rather than inverting twice. The preference alone is inert on a running session: IOHIDSystem reads it into `HIDParameters` at login only, and both the macOS driver and Karabiner follow `HIDFKeyMode`, not the plist, so `postActivation` runs `sunghyun fn-state apply` to push it in (`hidutil` cannot, it addresses HID devices and IOHIDSystem is not one) | `defaults read -g com.apple.keyboard.fnState` = 0; `ioreg -c IOHIDSystem` shows `HIDFKeyMode=0` (verify `check_fn_state`); `karabiner_cli --list-system-variables` reports `system.use_fkeys_as_standard_function_keys: false`; the top-row rule holds exactly four manipulators, f4 and f5 only, and `karabiner_cli --lint-complex-modifications` passes |
| t | Hyper+grave toggles the system appearance between light and dark, alternating on every press (owner 2026-08-08) | Karabiner manipulator → `sunghyun toggle-dark-mode`, which reads `SLSGetAppearanceThemeLegacy` and flips it with `SLSSetAppearanceThemeNotifying` (SkyLight). Not the usual Apple Event to System Events: sending that needs `kTCCServiceAppleEvents` for the sending process, a second consent prompt and a second privacy row on top of the Accessibility grant the binary already holds, whereas SkyLight talks to the window server with no TCC gate at all | manipulator present (verify `check_keyboard_engine`); `sunghyun toggle-dark-mode` flips `defaults read -g AppleInterfaceStyle` between `Dark` and absent |
| w | ⌘⇧Space belongs to 1Password Quick Access and nothing else (owner 2026-08-08) | macOS 27 ships symbolic hot key **263**, "Ask Siri about active window" (`screenshots.ask-siri-active-window`, key 49, modifiers 1179648), on the same chord, and it is a system default so it is absent from the preference domain until something disables it. `sunghyun hotkeys apply` (own module `nix/darwin/modules/hotkeys.nix`, run from `postActivation`) matches claimants by chord rather than by identifier, since Apple renumbers these between releases, then does both halves: `CGSSetSymbolicHotKeyEnabled` frees it in the running window server, and a read-modify-write of the single identifier inside `AppleSymbolicHotKeys` survives the next login. Never `CustomUserPreferences` for this key: it holds every other system shortcut | `sunghyun hotkeys status` reports no enabled claimant (verify `check_reserved_hotkeys`) |
| u | Bare fn (globe) tap opens the native macOS Emoji & Symbols picker, the same one Ctrl+Cmd+Space opens (owner 2026-08-08) | nix-darwin `system.defaults.hitoolbox.AppleFnUsageType = "Show Emoji & Symbols"` (writes `com.apple.HIToolbox AppleFnUsageType` = 2 for the primary user). This governs the bare tap only; fn held as a modifier rides `HIDFKeyMode`, so row o's fn+F-row inversion is untouched. Neither keyboard engine can eat the tap: karabiner.json uses fn only as a mandatory modifier on the f4/f5 chords, and kanata's defsrc never lists fn. HIToolbox reads the preference at process start, so a session that is already running keeps the old tap behaviour until the next login | `defaults read com.apple.HIToolbox AppleFnUsageType` = 2 (verify `check_fn_tap`) |
| l | **Hard invariant: keyboard never bricks.** Any keyboard-layer failure leaves typing working | Engine choice itself: Karabiner-Elements grabber seizes only with a healthy virtual device and its failure mode is "no remap", not "no keys" (2026-08-07 Kanata incident: root exclusive grab + dead VirtualHID output killed every key incl. the Accessibility Keyboard). Kanata remains an alternative engine behind `sunghyun kanata enable --safe` (mechanical passthrough/full proofs + rollback); its LaunchDaemon is flake-default OFF, `sunghyun kanata disable` always works and records a `launchctl disable` override so a plist rename cannot re-arm it | no root keyboard daemon by default; KE services running; typing unaffected when Input Monitoring is missing |

Engine evaluation (2026-08-07, per-outcome, no stack loyalty):

- **Karabiner-Elements** — chosen for a-e. Fully declarative JSON via Home
  Manager; nix-darwin ships `services.karabiner-elements`; same
  Karabiner-DriverKit-VirtualHIDDevice the machine already has approved; no
  root; a decade of production supervision (grabber/observer watchdog).
- **Kanata** — capable of a-e (tap-hold-press + danger-enable-cmd) but needs
  root, exclusive IOHID seize, Input Monitoring for a root binary, and custom
  proof/rollback scaffolding to stay safe; it bricked the keyboard on
  2026-08-07. Kept as opt-in alternative engine only.
- **hidutil / `system.keyboard.userKeyMapping`** — cannot express tap-hold;
  rejected for a, d; unused elsewhere.

## System

| # | Outcome | Implementation | Verify predicate |
|---|---------|----------------|------------------|
| f | Spotlight: ⌘Space enabled; typing "terminal" launches Ghostty | symbolichotkeys id 64 patched imperatively (whole-dict `defaults write` would clobber other shortcuts — no safe Nix path); Terminal→Ghostty alias app via `sunghyun` | verify `check_spotlight`, `check_terminal_alias` |
| g | Menu bar shows no date (Time Machine extra hidden; Cursor tray hidden) | nix-darwin `system.defaults.CustomUserPreferences` (systemuiserver); Cursor tray is app storage → `sunghyun` post-switch step | verify `check_menubar` |
| h | `~/.hushlogin` present | Home Manager `home.file` | verify `check_hushlogin` |
| i | Declared packages/apps present (nixpkgs + brews/casks, incl. CodexBar and the Hyper-layer apps Slack, IINA and Linear, all three as casks because the owner asked for brew over the App Store on 2026-08-08; mas: Xcode, KakaoTalk, What Watt?, Amphetamine) | nix-darwin `homebrew.{brews,casks}` + `environment.systemPackages`; mas apps via the convergence LaunchDaemon in row r, never from `brew bundle` (which hard-fails when the App Store is signed out) | `darwin-rebuild switch` succeeds; verify `check_apps` |
| j | zsh/dotfiles/runtimes configured, with no second repo to clone | zsh content is vendored in this repo (`assets/dotfiles/zsh/`); HM links `~/.zsh{env,rc,profile,login}` and `~/.config/zsh/{lib,rc,bin}` from the store, so the files are read-only and no vendor installer can append to them; HM must never generate rc content (`programs.zsh` off); runtimes via nixpkgs/brew | shell loads; `~/.zshrc` resolves into `/nix/store`, and a machine with no other repo cloned still converges |
| k | One-shot fresh-Mac bootstrap: single curl, sudo rarely (Touch ID via `security.pam.services.sudo_local`), zero babysitting | `install.sh` → Determinate Nix → `darwin-rebuild switch --flake` → `sunghyun post-switch` (opens Settings panes for one-time grants + polls) | fresh-machine run completes with ≤ the 3 known human toggles below |
| m | Headless/VM runs degrade gracefully (skips are not failures) | `SUNGHYUN_HEADLESS=1` + Aqua-session detection everywhere | headless `verify` / `post-switch` exit 0 |
| n | Everything idempotent and verifiable by outcome checks | `sunghyun verify` asserts outcomes, not implementation details | `sunghyun verify` exit 0 |
| p | Cursor Agent CLI (`cursor-agent`) present after the one-shot run, on macOS and on screenless Linux alike | macOS: official `cursor-cli` Homebrew cask declared in `homebrew.casks` (vendor release channel, stays writable so `cursor-agent update` works). Linux: nixpkgs `cursor-cli` in `nix/home/linux.nix`. Installing is the whole outcome — signing in is not automatable, see below | verify `check_cursor_agent` |
| v | Codex CLI (`codex`) and Claude Code CLI (`claude`) present after the one-shot run, on macOS and on screenless Linux alike | macOS: official `codex` and `claude-code` Homebrew casks declared in `homebrew.casks` (vendor release channels, stay writable for self-update). Linux: nixpkgs `codex` and `claude-code` in `nix/home/linux.nix`. Installing is the whole outcome. Sign-in (browser OAuth / API key) stays with the owner's session, same policy as row p, and the retired per-assistant instruction trees stay retired | verify `check_coding_cli` (`codex`, `claude`) |
| q | A machine keeps its own identity. Only the config that names a machine renames it | `nix/darwin/hosts/auracomputer.nix` is the sole config setting ComputerName/LocalHostName/HostName; every other Mac activates `.#default` (`nix/darwin/hosts/default.nix`), which sets none of them, and `install.sh` falls back to `default` rather than to a named host | `scutil --get ComputerName` is unchanged by a switch on a machine with no named host file |
| r | Mac App Store apps converge in the background after a later sign-in, without ever blocking or prompting; and inside a VM they skip by design | `launchd.daemons.masapps` (`com.anaclumos.masapps`): RunAtLoad + hourly StartInterval, logs to `/var/log/sunghyun-masapps.log`, boots itself out once every declared app is installed. Activation only writes the plist, so a switch is never slowed. Skips immediately under virtualization (row s); defers silently when no Apple Account is signed in; closes an App Store sign-in dialog and defers if one ever appears | `launchctl print system/com.anaclumos.masapps` after a switch; the log names skip/defer/converge with a reason |
| s | Virtualization is detected once, and the App Store surface skips because of it, not because of a timeout | `sunghyun virt` (`src/virt.rs`) — `kern.hv_vmm_present=1` primary, `hw.model` starting `VirtualMac` as an independent second witness. It is the only implementation; the mas daemon shells out to it rather than re-deriving the check | `sunghyun virt` exits 0 in a guest and 1 on metal; verify reports a `virtualization` line |
| v | Sunghyun Sans present, every family the font repo ships (Latin, KR, KR Hanja, JP, Disambiguated), on macOS and Linux alike | flake input `github:anaclumos/sunghyun-sans` (public, ships its own flake; the `default` package bundles all five families; pinned by rev + narHash in `flake.lock`). macOS: nix-darwin `fonts.packages` in `nix/darwin/modules/base.nix`, materialized under `/Library/Fonts/Nix Fonts`. Linux: `nix/home/fonts.nix` (`home.packages` + `fonts.fontconfig`), so fontconfig finds the profile fonts | verify `check_fonts`; `system_profiler SPFontsDataType` lists Sunghyun Sans after a switch |
| aa | Dia is installed | nix-darwin `homebrew.casks` entry `thebrowsercompany-dia`, the official homebrew/cask token (verified with `brew info --json=v2`, `.tap` = `homebrew/cask`), so no tap trust grant exists to abort a fresh `brew bundle` | verify `check_dia` |
| ab | Dia is the system default browser, so Hyper+J opens it | `sunghyun default-browser set`, run as a `post-switch` gate. `-[NSWorkspace setDefaultApplicationAtURL:toOpenURLsWithScheme:completionHandler:]` on the `http` scheme only (macOS derives https and HTML from it and rejects a direct https change). macOS raises its own confirmation panel for this change and has since 10.13; there is no way around it, so the step brings the panel forward and polls, and an unanswered panel is a skip the next switch retries | verify `check_default_browser` (LaunchServices http handler is `company.thebrowser.dia`) |
| ac | The Dock holds nothing but Finder and the Trash | nix-darwin `system.defaults.dock.persistent-apps = [ ]` / `persistent-others = [ ]` / `show-recents = false`. An empty list rather than the default `null`: nix-darwin drops null options from its `defaults write` list, so null is "unmanaged" and `[ ]` is written as an empty array. Finder and the Trash are Dock fixtures, not preferences, and macOS offers no way to remove them. Activation restarts the Dock itself whenever any dock default is written | verify `check_dock` |
| ad | Desktop shows hard disks, item info under each icon, and labels to the right of icons | `system.defaults.finder.ShowHardDrivesOnDesktop` is a plain key. The other two are not: they live in `com.apple.finder` → `DesktopViewSettings` → `IconViewSettings` (`showItemInfo`, and `labelOnBottom` inverted), which nix-darwin has no option for and `defaults write` cannot address by key path. A `postActivation` script therefore reads the dictionary with `defaults export` (not `defaults read`, whose old-style output turns every number and boolean into a string), edits only those two leaves with PlistBuddy, writes the dictionary back, and restarts Finder so cfprefsd cannot flush a stale cached copy over it | verify `check_desktop_icons` |
| ae | Temperature in Celsius, measurements metric | `system.defaults.NSGlobalDomain.AppleTemperatureUnit = "Celsius"`, `AppleMeasurementUnits = "Centimeters"`, `AppleMetricUnits = 1`. All three, because macOS reads all three and setting a subset leaves Settings metric while some formatters stay imperial. `AppleMetricUnits` is an integer enum at the pinned nix-darwin revision, not a boolean | verify `check_locale_units` |
| af | KakaoTalk runs in Korean whatever the system language is | Per-app `AppleLanguages` override, the same mechanism as the Applications list in Language & Region. Written by a `postActivation` script rather than `CustomUserPreferences`: KakaoTalk is a sandboxed App Store app, its preference domain redirects into `~/Library/Containers/com.kakao.KakaoTalkMac/Data/Library/Preferences`, and a write there is refused unless the writing process holds Full Disk Access. A typed nix-darwin default would abort activation on that refusal, so the script warns and moves on instead, and skips outright when the app is absent (fresh machine, signed-out App Store, VM) | verify `check_kakaotalk_language` (skips when the container is unreadable) |
| ag | Tailscale is installed, so this Mac can join the tailnet and MagicDNS hostnames resolve once signed in | nix-darwin `homebrew.casks` entry `tailscale-app`, the official homebrew/cask token (renamed from `tailscale`; verified with `brew info --json=v2`, `.tap` = `homebrew/cask`). This is the Standalone GUI variant, the one Tailscale recommends: its system Network Extension owns DNS natively so MagicDNS works without resolver wiring, and the app bundles daemon, GUI and CLI in one binary (`/Applications/Tailscale.app/Contents/MacOS/Tailscale`). Rejected: the `tailscale` formula, the CLI-only `tailscale`/`tailscaled` split that needs a root launchd daemon and manual DNS handling. Sign-in is browser OAuth, the same credential class as the Apple ID and Cursor Agent: install is the whole declarative outcome, and login stays with the owner's session. Linux stays out of the home layer on purpose: `tailscaled` is a system service and the headless Home Manager config has no root. Tailnet ACL tags live in the admin console, never in this repo | verify `check_tailscale` (`/Applications/Tailscale.app` present) |
| ah | tokenmaxxing (`tokenmaxxing`, alias `xx`) present after the one-shot run, on macOS and on screenless Linux alike (owner 2026-08-08) | flake input `github:anaclumos/tokenmaxxing` (public, ships its own flake; pinned by rev + narHash in `flake.lock`). macOS: `darwinModules.withOverlay` + `programs.tokenmaxxing.enable` in `flake.nix` (systemPackages). Linux: `homeManagerModules.default` in `nix/home/linux.nix` with the input's package set explicitly, because the standalone Home Manager pkgs carries no overlay. Installing the CLI is the whole outcome: `tokenmaxxing init` owns accounts, the `claude` supervisor shim and settings hooks, and stays with the owner (retired shim surfaces stay retired, same policy as row v), and the module's optional check timer keeps its default of off | verify `check_tokenmaxxing` |
| ai | Desktop icons snap to grid | Finder's Snap to Grid is `arrangeBy = "grid"` inside `com.apple.finder` → `DesktopViewSettings` → `IconViewSettings`. `"none"` is free placement, and a sort key like `"name"` is the stronger Sort By, which was not asked for. No nix-darwin option exists at the pinned revision (no match for `DesktopViewSettings` or `arrangeBy` anywhere in its defaults modules), so the row-ad `postActivation` script edits this leaf alongside its two and restarts Finder | verify `check_desktop_icons` |

## Known human surfaces (final policy, owner 2026-08-07)

Verified against the full nix-darwin manual 2026-08-07: no options exist for
TCC grants, system-extension approval, or App Store sign-in (SIP-protected
TCC.db; PPPC profiles are MDM-only).

**TCC/dext**: the sanctioned surface is open-window-and-poll. The system
triggers the OS's own prompt or opens the exact Settings pane
(`x-apple.systempreferences` deep links), the owner clicks the toggle, the
system polls with a sane timeout, and a timeout is a graceful skip that
converges on the next switch. No instruction text, no stdin prompts, no
agent-driven clicking (the earlier CUA-gate framework is superseded).

**Apple ID**: assumed from Setup Assistant. If signed out, mas apps skip
gracefully (never block, loop, or instruct) and converge on a later switch.

**Virtualization** (owner, 2026-08-08): inside a VM the Mac App Store surface
skips by design. A guest is never signed into a real Apple Account in an
end-to-end demo, so `mas` reports a skip naming virtualization, App Store is
never launched, and the convergence daemon retires immediately instead of
retrying. The gate stops there on purpose: it covers what a VM can never
satisfy, not merely what an unattended run cannot. TCC and dext panes stay on
the open-window-and-poll path even in a guest, because a person at the guest's
console can grant them, and a timed-out pane costs nothing but time — while a
mas attempt leaves a modal sign-in sheet on screen and a background job wedged
behind it.

**Cursor Agent sign-in is not automatable.** The one-shot run installs
`cursor-agent`; it cannot authenticate it. `agent login` is a browser OAuth
flow, and the only alternative is a `CURSOR_API_KEY` from the Cursor dashboard
(<https://cursor.com/docs/cli/reference/authentication>). Both are credentials
a script must not invent, which puts them in the same class as the Apple ID:
install, then let the owner's existing session take over. There is therefore no unattended "Cursor Agent GUI
continuation" step, and the superseded CUA gate framework is not coming back.

Exactly three first-boot toggles remain, each in a window the system opens:

1. Karabiner DriverKit dext approval (Login Items & Extensions pane)
2. Input Monitoring / Accessibility for the keyboard engine (macOS's own
   prompt when Karabiner-Elements first launches)
3. Accessibility for `/usr/local/bin/sunghyun` (Privacy & Security pane, for
   tiling; the grant is per code identity, so a rebuilt binary re-runs this
   toggle via post-switch open-pane-and-poll)

**Administration dialogs (macOS 26, measured on a pristine guest 2026-08-08).**
A first boot also raises two blocking dialogs titled "Terminal would like to
administer your computer": the first when the Determinate installer creates the
encrypted `/nix` APFS volume, the second during nix-darwin activation. They are
Allow-or-deny clicks with no typing, and macOS raises them against the
responsible app (whichever terminal ran the one-liner), so nothing in this repo
suppresses them and no installer flag avoids them. Budget them alongside the one
sudo password: a first run is one password plus two clicks. A second run on the
same machine raised neither dialog, though that run also had no volume to create
and no activation work to do, so it does not by itself prove the grant is
remembered.
