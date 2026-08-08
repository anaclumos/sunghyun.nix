---
name: npm-clis-nix-native-over-bun-activation
description: npm-distributed CLIs ship as proper Nix packages, not imperative bun-global activation steps
metadata:
  type: feedback
---

Owner default (2026-08-08): bun is the preferred runner for global npm package installs in general. Refinement the same day for this repo: when a cleanly better Nix-native mechanism exists, use it instead; the bar is the most succinct and elegant declaration with cheap version bumps. An imperative `bun add --global` inside a home-manager activation script was retired in favor of researched Nix packaging (nixpkgs guidance recommends `buildNpmPackage`-family builders). Correction source: review hook, then owner confirmation.

**Why:** Activation-script installs are unpinned and mutable, which defeats the declarative repo model; the nixpkgs manual documents supported builders for npm projects.

**How to apply:** For an npm-only CLI, research the packaging options (buildNpmPackage, importNpmLock, bundled-tarball wrapper) and pick the smallest pure declaration before reaching for bun. Related: [[todo-inventories-are-read-only]].
