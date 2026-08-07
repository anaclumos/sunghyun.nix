# Root LaunchDaemon for Kanata (DriverKit VirtualHID IPC is root-only).
#
# Do NOT enable any TCC.db sqlite hack. Human Input Monitoring + Accessibility
# grants stay in `sunghyun post-switch` / verify.
#
# DriverKit: pin Karabiner-DriverKit-VirtualHIDDevice v6.2.0 (Kanata requirement).
# First activation always needs a System Settings click; Nix cannot click it.
# Prefer brew "kanata" (cmd_allowed) at /opt/homebrew/bin/kanata.
#
# Toggle: services.sunghyun.kanata.enable
# Default OFF so bare `darwin-rebuild` cannot brick the keyboard.
# Product enable path is runtime: `sunghyun kanata enable --safe`
# (VirtualHID + passthrough proof + rollback), invoked by install.sh.
#
# Boot-load hardening (2026-08-08): `sunghyun kanata disable` records a
# `launchctl disable system/com.anaclumos.kanata` override, so renaming the
# parked plist back can never re-arm the daemon without the safe-enable gate
# (which runs `launchctl enable` itself). The declarative path below clears
# the override too, since `launchctl bootstrap` refuses disabled services.
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

  config = lib.mkIf cfg.kanata.enable {
    # extraActivation because arbitrary activationScripts.<name> entries are
    # silently ignored by nix-darwin; types.lines concatenates across modules.
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
  };
}
