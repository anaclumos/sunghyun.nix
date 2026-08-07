# Mirrors assets/Brewfile + assets/manifest.toml (mas).
# cleanup = "none" so unmanaged formulae/casks are not removed on switch.
{
  homebrew = {
    enable = true;
    onActivation = {
      autoUpdate = false;
      upgrade = false;
      # Never "uninstall"/"zap": the karabiner-elements cask uninstall script
      # purges the shared DriverKit VirtualHID daemon files.
      cleanup = "none";
      # Homebrew 6.x ask-mode / hints must never stall root activation.
      extraEnv = {
        HOMEBREW_NO_ASK = "1";
        HOMEBREW_NO_ENV_HINTS = "1";
      };
    };
    brews = [
      "fnm"
      "gh"
      "mas"
      "mole"
      "ripgrep"
      "tmux"
      "topgrade"
      # Alternative keyboard engine (OUTCOMES.md); daemon flake-default OFF.
      "kanata"
    ];
    casks = [
      "1password"
      "cursor"
      # Cursor Agent CLI (binary: cursor-agent). Official homebrew/cask
      # formula, so it needs no `brew trust` grant, tracks the vendor's own
      # release channel, and stays writable so `cursor-agent update` works —
      # none of which a Nix store copy can do. Linux gets nixpkgs cursor-cli
      # from nix/home/linux.nix instead.
      "cursor-cli"
      "ghostty"
      "itsycal"
      # Primary keyboard engine (OUTCOMES.md a-e). Cask (KE 16.x), not
      # nix-darwin services.karabiner-elements: that module is broken for
      # KE >= 15 (nix-darwin#1041) and nixpkgs lags; KE 16 also folds Input
      # Monitoring into Accessibility, shrinking the TCC residue.
      # NEVER `brew uninstall --cask karabiner-elements` (deletes the
      # DriverKit dext Kanata/KE share).
      "karabiner-elements"
    ];
    # No masApps here on purpose: brew bundle hard-fails activation when the
    # App Store is signed out. Owner policy (2026-08-07): Apple ID comes from
    # Setup Assistant; if signed out, mas apps must skip gracefully and
    # converge on a later switch. See masapps.nix.
  };

  # Mac App Store apps, non-fatal and non-blocking: installs run detached so a
  # multi-GB download (Xcode) never stalls activation, and a signed-out App
  # Store just fails quietly in the log (mas 5+ has no CLI sign-in; never
  # block or instruct) — either way it converges on a later switch.
  # postActivation is one of the hooks nix-darwin actually executes (arbitrary
  # activationScripts names are not) and runs after homebrew installs mas.
  # mas must run as the primary user, not pure root: as root without SUDO_UID
  # it dies with "Failed to get sudo uid" (that is how brew bundle invokes it
  # too). sudo -u from root activation needs no password.
  system.activationScripts.postActivation.text = ''
    MAS=/opt/homebrew/bin/mas
    if [ -x "$MAS" ]; then
      nohup /usr/bin/sudo -u sc -H /bin/sh -c '
        for id in 497799835 869223134 6756065687 937984704; do # Xcode, KakaoTalk, What Watt?, Amphetamine
          "'"$MAS"'" list 2>/dev/null | grep -q "^$id " && continue
          "'"$MAS"'" install "$id" \
            || echo "masApps: $id skipped (App Store signed out?); converges next switch"
        done
      ' >/var/log/sunghyun-masapps.log 2>&1 &
      echo "masApps: converging in background (log: /var/log/sunghyun-masapps.log)"
    else
      echo "masApps: mas not installed yet; converges next switch"
    fi
  '';
}
