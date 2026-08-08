# Default OFF: a grabbing Kanata without healthy VirtualHID output drops every
# key, so a bare `darwin-rebuild` must never start it. The only enable path is
# `sunghyun kanata enable --safe`.
{
  config,
  lib,
  ...
}:
let
  cfg = config.services.sunghyun;
  home = config.users.users.${config.system.primaryUser}.home;
  kbd = "${home}/.config/sunghyun/kanata.kbd";
  logDir = "${home}/Library/Logs/sunghyun";
in
{
  options.services.sunghyun.kanata = {
    enable = lib.mkEnableOption "root LaunchDaemon for Kanata (sunghyun keyboard stack)" // {
      default = false;
    };
    packagePath = lib.mkOption {
      type = lib.types.path;
      default = "/opt/homebrew/bin/kanata";
      description = "Absolute path to a cmd_allowed kanata binary (usually Homebrew).";
    };
  };

  config = lib.mkMerge [
    (lib.mkIf (!cfg.kanata.enable) {
      # Persisted every activation while the engine is off, because the runtime
      # `kanata disable` path never runs when the daemon is already parked, and
      # without the override a bare plist rename re-arms it at boot.
      system.activationScripts.extraActivation.text = lib.mkAfter ''
        launchctl disable system/com.anaclumos.kanata || true
      '';
    })
    (lib.mkIf cfg.kanata.enable {
    system.activationScripts.extraActivation.text = lib.mkAfter ''
      mkdir -p ${lib.escapeShellArg logDir}
      chown ${config.system.primaryUser}:staff ${lib.escapeShellArg logDir} || true
      launchctl enable system/com.anaclumos.kanata || true
    '';

    launchd.daemons."com.anaclumos.kanata" = {
      command = "${cfg.kanata.packagePath} --cfg ${kbd} --no-wait";
      serviceConfig = {
        Label = "com.anaclumos.kanata";
        UserName = "root";
        RunAtLoad = true;
        KeepAlive = {
          SuccessfulExit = false;
        };
        EnvironmentVariables = {
          PATH = "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin";
        };
        StandardOutPath = "${logDir}/kanata.out.log";
        StandardErrorPath = "${logDir}/kanata.err.log";
      };
    };
    })
  ];
}
