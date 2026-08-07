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
`homeConfigurations."sc@linux"` output (portable layer only). Module hygiene
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
| c | Hyper+J = open system default browser (H/I/K/L/M/N = mail/linear/music/calendar/kakaotalk/slack; D = desktop; F = Mission Control) | same, `shell_command → /usr/local/bin/sunghyun` | manipulators present; `sunghyun open-default-browser` resolves |
| d | Left ⌘ tap = ABC IME, Right ⌘ tap = 2-Set Korean; held = normal ⌘ chords (⌘C/⌘V must never break) | Karabiner `to_if_alone` + lazy `left_command`/`right_command` | manipulators present; `sunghyun input-source` works (verify `check_ime_mapping`) |
| e | ⌘⇧V = Spotlight clipboard history | Karabiner manipulator sends virtual ⌘Space then ⌘4 as HID key events (no shell hop, no TCC; macOS 27 gates synthetic keystrokes from spawned processes beyond Accessibility) | manipulator present; pasteboard history enabled (verify `check_spotlight_clipboard`) |
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
| i | Declared packages/apps present (nixpkgs + brews/casks + mas: Xcode, KakaoTalk, What Watt?, Amphetamine) | nix-darwin `homebrew.{brews,casks}` + `environment.systemPackages`; mas apps via non-fatal `postActivation` script (signed out ⇒ warn + skip, converge next switch) | `darwin-rebuild switch` succeeds; verify `check_apps` |
| j | zsh/dotfiles/runtimes configured | HM owns the `~/.zsh*` symlink wiring (out-of-store links into `~/Developer/configs/zsh/`, the single canonical clone; install.sh ensures it); content stays owned by anaclumos/configs; HM must never generate rc content (`programs.zsh` off); runtimes via nixpkgs/brew | shell loads; `~/.zshrc` resolves into `~/Developer/configs` |
| k | One-shot fresh-Mac bootstrap: single curl, sudo rarely (Touch ID via `security.pam.services.sudo_local`), zero babysitting | `install.sh` → Determinate Nix → `darwin-rebuild switch --flake` → `sunghyun post-switch` (opens Settings panes for one-time grants + polls) | fresh-machine run completes with ≤ the 3 known human toggles below |
| m | Headless/VM runs degrade gracefully (skips are not failures) | `SUNGHYUN_HEADLESS=1` + Aqua-session detection everywhere | headless `verify` / `post-switch` exit 0 |
| n | Everything idempotent and verifiable by outcome checks | `sunghyun verify` asserts outcomes, not implementation details | `sunghyun verify` exit 0 |

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

Exactly three first-boot toggles remain, each in a window the system opens:

1. Karabiner DriverKit dext approval (Login Items & Extensions pane)
2. Input Monitoring / Accessibility for the keyboard engine (macOS's own
   prompt when Karabiner-Elements first launches)
3. Accessibility for `/usr/local/bin/sunghyun` (Privacy & Security pane, for
   tiling; the grant is per code identity, so a rebuilt binary re-runs this
   toggle via post-switch open-pane-and-poll)
