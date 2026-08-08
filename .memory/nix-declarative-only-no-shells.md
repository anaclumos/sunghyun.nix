---
name: nix-declarative-only-no-shells
description: The nix tree is declarative-only, zero comments and zero runtime shell; runtime code lives in Hammerspoon
metadata:
  type: feedback
---

Owner mandate (2026-08-08): "No comments. No shells. Rewrite everything in the most compact Nix way." The repo carries zero comments in nix files (including inside embedded strings) and zero runtime shell: no writeShellScript, no activationScripts, no home.activation, no shell launchd payloads, no tracked .sh files. The sunghyun CLI, install.sh, Kanata engine and every converge script were deleted under this rule.

**Why:** Imperative converge machinery fights the declarative model and rots; the owner wants the flake to be the whole story.

**How to apply:** Express state through nix-darwin and Home Manager options (homebrew.masApps, CustomUserPreferences). When something genuinely needs runtime code, host it in Hammerspoon: Lua in the generated init.lua, private-framework calls as in-process JXA via hs.osascript.javascript, hotkeys via hs.hotkey.bind on real-modifier chords emitted by karabiner.json (native software_function.open_application for app launches). Derivation build phases (installPhase) remain acceptable, that is standard Nix packaging. Outcomes with no declarative path stay as live converged state documented in README rows, restored by hand on a fresh machine. See [[npm-clis-nix-native-over-bun-activation]].
