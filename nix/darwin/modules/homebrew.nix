# Mirrors assets/Brewfile + assets/manifest.toml (mas).
# cleanup = "none" so unmanaged formulae/casks are not removed on switch.
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

  # Mirrors assets/manifest.toml [[mas_apps]].
  masApps = {
    "497799835" = "Xcode";
    "869223134" = "KakaoTalk";
    "6756065687" = "What Watt?";
    "937984704" = "Amphetamine";
  };
  masAppIds = lib.concatStringsSep " " (lib.attrNames masApps);

  masLog = "/var/log/sunghyun-masapps.log";
  masLabel = "com.anaclumos.masapps";

  # Convergence job for Mac App Store apps. See the LaunchDaemon comment below
  # for why this is a daemon and not an activation child process.
  masConverge = pkgs.writeShellScript "sunghyun-masapps-converge" ''
    set -u
    PATH=/usr/bin:/bin:/usr/sbin:/sbin
    MAS=/opt/homebrew/bin/mas
    LABEL=${masLabel}

    log() { printf '%s masapps: %s\n' "$(date -u +%FT%TZ)" "$*"; }

    # Stop this job for good (until the next `darwin-rebuild switch` writes a
    # new plist, which re-bootstraps it). `bootout` and not `disable`: a
    # persisted launchctl override would also block the re-bootstrap, so a
    # changed app list could never converge again.
    retire() {
      log "$1"
      /bin/launchctl bootout "system/$LABEL" 2>/dev/null || true
      exit 0
    }

    # Virtualization: skip by design, never by timeout (owner, 2026-08-08).
    # A guest may not be signed into a real Apple Account, so there is nothing
    # here to converge — and `mas install` would open an App Store sign-in
    # dialog that no one can answer. Single source of truth: `sunghyun virt`.
    VIRT="$(${sunghyun}/bin/sunghyun virt 2>&1)"
    if [ $? -eq 0 ]; then
      retire "skipped by design: $VIRT"
    fi

    if [ ! -x "$MAS" ]; then
      log "mas not installed yet; retrying on the next interval"
      exit 0
    fi

    # mas needs the primary user's Apple Account context. Run as root (mas
    # requires root for `install` and would otherwise prompt for the macOS
    # password to escalate itself) with SUDO_UID/SUDO_USER set, which is the
    # invocation `brew bundle` makes and the one mas expects; without
    # SUDO_UID it exits with "Failed to get sudo uid".
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

    # Signed-out probe that costs nothing and opens nothing. `mas account` was
    # removed upstream (unsupported since macOS 12), and `mas install` on a
    # signed-out Mac opens an App Store sign-in dialog and blocks — which is
    # exactly the sheet that used to be left on screen. An Apple Account
    # signed in through Setup Assistant materializes MobileMeAccounts, so its
    # absence is a cheap, GUI-free "not signed in yet" signal. Failing this
    # probe defers; it never prompts and never fails the switch.
    ACCOUNTS="/Users/$USER_NAME/Library/Preferences/MobileMeAccounts.plist"
    if ! /usr/libexec/PlistBuddy -c "Print :Accounts:0:AccountID" "$ACCOUNTS" >/dev/null 2>&1; then
      log "no Apple Account signed in yet; deferring$missing (retrying hourly, no dialog)"
      exit 0
    fi

    # Last line of defence for the dialog. MobileMeAccounts proves an iCloud
    # account, which is the same account as Media & Purchases on the Setup
    # Assistant path but not necessarily on a hand-configured Mac. In that one
    # mismatch case `mas install` still opens App Store's sign-in sheet and
    # blocks on it forever — the sheet that used to be left on screen after a
    # run. A normal install never launches App Store.app, so its appearance is
    # the dialog: close it, stop the attempt, and try again later. An App
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
    # converge on a later switch.
  };

  # Mac App Store convergence runs as a supervised LaunchDaemon, not as a
  # `nohup … &` child of the activation script.
  #
  # The old shape never converged: activation runs under `sudo`, and sudo
  # 1.9.14+ defaults to `use_pty`, so the command gets its own pty session and
  # sudo tears down what is left in it when activation returns. The detached
  # mas process died with that teardown before writing a byte — a 0-length
  # /var/log/sunghyun-masapps.log and no surviving process, observed on the
  # 2026-08-07 VM run. Worse, `mas install` on a signed-out Mac opens an App
  # Store sign-in dialog and blocks on it, which is what left a sign-in sheet
  # on screen after the run finished.
  #
  # launchd is the supported way to own a retrying background job: it survives
  # activation, restarts the script every StartInterval, keeps its own log, and
  # the script boots itself out the moment every app is installed. Nothing here
  # can block or slow a switch, because activation only writes the plist.
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
      # Never restart-loop: this job is meant to exit, and to exit for good
      # once converged.
      KeepAlive = false;
      LowPriorityIO = true;
    };
  };
}
