{
  config,
  lib,
  pkgs,
  self,
  ...
}:
let
  primaryUser = config.system.primaryUser;
  sunghyun = self.packages.${pkgs.stdenv.hostPlatform.system}.sunghyun;

  masApps = {
    "497799835" = "Xcode";
    "869223134" = "KakaoTalk";
    "6756065687" = "What Watt?";
    "937984704" = "Amphetamine";
  };
  masAppIds = lib.concatStringsSep " " (lib.attrNames masApps);

  masLog = "/var/log/sunghyun-masapps.log";
  masLabel = "com.anaclumos.masapps";

  masConverge = pkgs.writeShellScript "sunghyun-masapps-converge" ''
    set -u
    PATH=/usr/bin:/bin:/usr/sbin:/sbin
    MAS=/opt/homebrew/bin/mas
    LABEL=${masLabel}

    log() { printf '%s masapps: %s\n' "$(date -u +%FT%TZ)" "$*"; }

    # `bootout` and not `disable`: a persisted override would also block the
    # re-bootstrap, so a changed app list could never converge again.
    retire() {
      log "$1"
      /bin/launchctl bootout "system/$LABEL" 2>/dev/null || true
      exit 0
    }

    # A guest may have no real Apple Account, and `mas install` would then open
    # a sign-in dialog no one can answer: skip by design, never by timeout.
    VIRT="$(${sunghyun}/bin/sunghyun virt 2>&1)"
    if [ $? -eq 0 ]; then
      retire "skipped by design: $VIRT"
    fi

    if [ ! -x "$MAS" ]; then
      log "mas not installed yet; retrying on the next interval"
      exit 0
    fi

    # mas needs root for `install` plus the primary user's Apple Account
    # context: without SUDO_UID it exits with "Failed to get sudo uid".
    USER_NAME=${lib.escapeShellArg primaryUser}
    USER_UID="$(/usr/bin/id -u "$USER_NAME" 2>/dev/null || true)"
    USER_GID="$(/usr/bin/id -g "$USER_NAME" 2>/dev/null || true)"
    if [ -z "$USER_UID" ]; then
      log "primary user $USER_NAME has no uid yet; retrying on the next interval"
      exit 0
    fi
    export SUDO_USER="$USER_NAME" SUDO_UID="$USER_UID" SUDO_GID="$USER_GID"

    installed="$("$MAS" list 2>/dev/null || true)"
    missing=""
    for id in ${masAppIds}; do
      printf '%s\n' "$installed" | /usr/bin/grep -q "^$id " || missing="$missing $id"
    done
    if [ -z "$missing" ]; then
      retire "converged: every declared App Store app is installed"
    fi

    # `mas account` was removed upstream, so this plist is the only GUI-free
    # signed-out probe: Setup Assistant sign-in materializes it.
    ACCOUNTS="/Users/$USER_NAME/Library/Preferences/MobileMeAccounts.plist"
    if ! /usr/libexec/PlistBuddy -c "Print :Accounts:0:AccountID" "$ACCOUNTS" >/dev/null 2>&1; then
      log "no Apple Account signed in yet; deferring$missing (retrying hourly, no dialog)"
      exit 0
    fi

    # iCloud and Media & Purchases can be different accounts, and in that case
    # `mas install` still blocks on a sign-in sheet forever. A normal install
    # never launches App Store.app, so its appearance is that sheet. An App
    # Store the owner opened themselves is left alone.
    appstore_was_open=0
    /usr/bin/pgrep -x "App Store" >/dev/null 2>&1 && appstore_was_open=1

    for id in $missing; do
      log "installing $id"
      "$MAS" install "$id" &
      mas_pid=$!
      dialog_seen=0
      while /bin/kill -0 "$mas_pid" 2>/dev/null; do
        if [ "$appstore_was_open" -eq 0 ] && /usr/bin/pgrep -x "App Store" >/dev/null 2>&1; then
          dialog_seen=1
          /bin/kill "$mas_pid" 2>/dev/null || true
          /usr/bin/pkill -x "App Store" 2>/dev/null || true
          break
        fi
        /bin/sleep 10
      done
      wait "$mas_pid"
      rc=$?
      if [ "$dialog_seen" -eq 1 ]; then
        log "App Store opened a sign-in dialog for $id; closed it, stopping this pass (retrying on the next interval)"
        exit 0
      elif [ "$rc" -eq 0 ]; then
        log "installed $id"
      else
        log "install $id failed (rc=$rc); retrying on the next interval"
      fi
    done

    installed="$("$MAS" list 2>/dev/null || true)"
    still=""
    for id in ${masAppIds}; do
      printf '%s\n' "$installed" | /usr/bin/grep -q "^$id " || still="$still $id"
    done
    if [ -z "$still" ]; then
      retire "converged: every declared App Store app is installed"
    fi
    log "still missing:$still; retrying on the next interval"
  '';
in
{
  # With cleanup = "uninstall", deleting the karabiner-elements line would make
  # the next activation run its cask uninstall script, which purges the shared
  # DriverKit VirtualHID files Karabiner-Elements and Kanata both need, and a
  # machine with no virtual device output eats every key including the
  # on-screen keyboard. Removing it must be a build failure, never a switch.
  assertions = [
    {
      assertion = lib.elem "karabiner-elements" (map (c: c.name) config.homebrew.casks);
      message = "homebrew.casks must keep karabiner-elements: onActivation.cleanup = \"uninstall\" would run its cask uninstall script, which deletes the shared Karabiner DriverKit VirtualHIDDevice files and bricks the keyboard. If it truly has to go, handle the DriverKit daemon migration by hand first.";
    }
  ];

  # The declared set, at the path verify reads. Not global.brewfile's env var:
  # a shell opened before the switch would keep pointing at the previous
  # generation's Brewfile, while /etc tracks the active generation.
  environment.etc."sunghyun/Brewfile".text = config.homebrew.brewfile;

  homebrew = {
    enable = true;
    onActivation = {
      autoUpdate = false;
      upgrade = false;
      # Owner 2026-08-08: a definition deleted from this repo must be gone from
      # the machine on the next switch. Not "zap": its deeper clean needs Full
      # Disk Access for the process running brew, activation under sudo does
      # not hold FDA, and a failed zap strands the cask half-uninstalled.
      cleanup = "uninstall";
      # Homebrew 6.x prompts y/n by default, which would stall root activation.
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
      "kanata"
    ];
    casks = [
      "1password"
      "aside"
      "claude-code"
      "codex"
      "codexbar"
      "cursor"
      # Stays writable so `cursor-agent update` works, which a Nix store copy
      # cannot do. Linux gets nixpkgs cursor-cli from nix/home/linux.nix.
      "cursor-cli"
      "ghostty"
      "iina"
      "itsycal"
      # Cask, not nix-darwin services.karabiner-elements: that module is broken
      # for KE >= 15 (nix-darwin#1041). NEVER `brew uninstall --cask
      # karabiner-elements`; it deletes the DriverKit dext Kanata and KE share.
      "karabiner-elements"
      "linear"
      "slack"
      # Standalone GUI variant (system Network Extension), so MagicDNS DNS
      # config is native; the `tailscale` formula is the CLI-only tailscaled
      # split that needs root launchd wiring. Token renamed from `tailscale`.
      "tailscale-app"
    ];
    # No masApps here on purpose: `brew bundle` hard-fails activation when the
    # App Store is signed out, so they converge from the daemon below instead.
  };

  # A LaunchDaemon and not an activation child process: activation runs under
  # sudo with use_pty, so a detached child dies with the pty teardown when
  # activation returns (0-length log, no process).
  launchd.daemons.masapps = {
    # `command` (not ProgramArguments) so nix-darwin wraps it in
    # `/bin/wait4path /nix/store`: at boot the daemon can otherwise fire
    # before the Nix store is mounted.
    command = "${masConverge}";
    serviceConfig = {
      Label = masLabel;
      RunAtLoad = true;
      StartInterval = 3600;
      ProcessType = "Background";
      StandardOutPath = masLog;
      StandardErrorPath = masLog;
      KeepAlive = false;
      LowPriorityIO = true;
    };
  };
}
