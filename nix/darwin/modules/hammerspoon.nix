{
  config,
  lib,
  ...
}:
let
  primaryUser = config.system.primaryUser;
  home = config.users.users.${primaryUser}.home;
  app = "/Applications/Hammerspoon.app";
in
{
  homebrew.casks = [ "hammerspoon" ];

  launchd.user.agents.hammerspoon = {
    serviceConfig = {
      Label = "com.anaclumos.hammerspoon";
      ProgramArguments = [ "${app}/Contents/MacOS/Hammerspoon" ];
      RunAtLoad = true;
      # Restart a crash, respect a deliberate quit.
      KeepAlive.SuccessfulExit = false;
      ProcessType = "Interactive";
      StandardOutPath = "${home}/Library/Logs/sunghyun/hammerspoon.out.log";
      StandardErrorPath = "${home}/Library/Logs/sunghyun/hammerspoon.err.log";
    };
  };

  system.activationScripts.postActivation.text = lib.mkAfter ''
    install -d -o ${lib.escapeShellArg primaryUser} -g staff ${lib.escapeShellArg "${home}/Library/Logs/sunghyun"}
    if [ -d ${lib.escapeShellArg app} ]; then
      hsUid="$(id -u -- ${lib.escapeShellArg primaryUser})"
      # -k, not a plain kickstart: init.lua is a symlink into the store, so a
      # switch replaces the target under a Hammerspoon that already read it, and
      # a process started before the link existed has no pathwatcher to notice.
      launchctl asuser "$hsUid" sudo --user=${lib.escapeShellArg primaryUser} -- \
        /bin/launchctl kickstart -k "gui/$hsUid/com.anaclumos.hammerspoon" >/dev/null 2>&1 || true
    else
      echo >&2 "sunghyun: Hammerspoon not installed yet (cask lands on this switch); tiling converges on the next one"
    fi
  '';
}
