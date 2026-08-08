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

    retire() {
      log "$1"
      /bin/launchctl bootout "system/$LABEL" 2>/dev/null || true
      exit 0
    }

    VIRT="$(${sunghyun}/bin/sunghyun virt 2>&1)"
    if [ $? -eq 0 ]; then
      retire "skipped by design: $VIRT"
    fi

    if [ ! -x "$MAS" ]; then
      log "mas not installed yet; retrying on the next interval"
      exit 0
    fi

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

    ACCOUNTS="/Users/$USER_NAME/Library/Preferences/MobileMeAccounts.plist"
    if ! /usr/libexec/PlistBuddy -c "Print :Accounts:0:AccountID" "$ACCOUNTS" >/dev/null 2>&1; then
      log "no Apple Account signed in yet; deferring$missing (retrying hourly, no dialog)"
      exit 0
    fi

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
  assertions = [
    {
      assertion = lib.elem "karabiner-elements" (map (c: c.name) config.homebrew.casks);
      message = "homebrew.casks must keep karabiner-elements: onActivation.cleanup = \"uninstall\" would run its cask uninstall script, which deletes the shared Karabiner DriverKit VirtualHIDDevice files and bricks the keyboard. If it truly has to go, handle the DriverKit daemon migration by hand first.";
    }
  ];

  environment.etc."sunghyun/Brewfile".text = config.homebrew.brewfile;

  homebrew = {
    enable = true;
    taps = [ "getsentry/tools" ];
    onActivation = {
      autoUpdate = true;
      upgrade = true;
      cleanup = "uninstall";
      extraEnv = {
        HOMEBREW_NO_ASK = "1";
        HOMEBREW_NO_ENV_HINTS = "1";
      };
    };
    brews = [
      "fnm"
      "gh"
      "kanata"
      "mas"
      "mole"
      "pscale"
      "ripgrep"
      "getsentry/tools/sentry"
      "stripe-cli"
      "tmux"
      "vercel"
    ];
    casks = [
      "1password"
      "aside"
      "claude-code"
      "codex"
      "codexbar"
      "cursor"
      "cursor-cli"
      "gcloud-cli"
      "ghostty"
      "iina"
      "itsycal"
      "karabiner-elements"
      "linear"
      "macs-fan-control"
      "obsidian"
      "orbstack"
      "slack"
      "tailscale-app"
    ];
  };

  launchd.daemons.masapps = {
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
