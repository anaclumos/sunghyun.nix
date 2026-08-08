{
  config,
  lib,
  pkgs,
  self,
  ...
}:
let
  cfg = config.services.sunghyun;
  package = self.packages.${pkgs.stdenv.hostPlatform.system}.sunghyun;
  home = config.users.users.${config.system.primaryUser}.home;
in
{
  options.services.sunghyun = {
    enable = lib.mkEnableOption "sunghyun CLI (verify, post-switch, kanata safe-enable)" // {
      default = true;
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ package ];

    system.activationScripts.extraActivation.text = ''
      if [ -e /usr/local/bin/sunghyun ] && ! [ -L /usr/local/bin/sunghyun ]; then
        echo "sunghyun: removing the compiled binary staged at /usr/local/bin (Hammerspoon owns Accessibility now)"
        rm -f /usr/local/bin/sunghyun
      fi
      if /usr/bin/security find-identity -p codesigning /Library/Keychains/System.keychain 2>/dev/null | grep -q '"sunghyun-codesign"'; then
        echo "sunghyun: deleting the sunghyun-codesign identity (nothing is signed with it any more)"
        /usr/bin/security delete-identity -c sunghyun-codesign /Library/Keychains/System.keychain >/dev/null 2>&1 || true
      fi

      rm -f /usr/local/bin/sunghyun-os
      rm -f ${lib.escapeShellArg home}/.local/bin/sunghyun-os
      rm -f ${lib.escapeShellArg home}/.cargo/bin/sunghyun-os
    '';
  };
}
