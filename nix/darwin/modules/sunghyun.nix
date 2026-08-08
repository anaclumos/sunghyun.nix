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

    # extraActivation is a hook nix-darwin actually runs (arbitrary
    # activationScripts.<name> entries are ignored by the activate script).
    system.activationScripts.extraActivation.text = ''
      echo "sunghyun: staging stable CLI copy at /usr/local/bin (TCC path stability)"
      mkdir -p /usr/local/bin
      install -m 755 ${package}/bin/sunghyun /usr/local/bin/sunghyun

      # TCC pins a grant to the binary's designated requirement. An ad-hoc
      # signature's DR is `cdhash H"..."`, so every rebuild minted a NEW
      # Accessibility identity (five duplicate "sunghyun" rows in Settings,
      # observed live 2026-08-08). Signing with a local self-signed cert
      # shifts the DR to `identifier ... and certificate leaf = H"..."`,
      # stable across rebuilds (the yabai/skhd pattern). The identity is
      # created on first activation; `security find-identity` must run
      # WITHOUT -v, which hides untrusted-but-usable self-signed certs.
      if ! /usr/bin/security find-identity -p codesigning /Library/Keychains/System.keychain 2>/dev/null | grep -q '"sunghyun-codesign"'; then
        echo "sunghyun: creating sunghyun-codesign identity in the System keychain"
        certdir=$(mktemp -d)
        /usr/bin/openssl req -x509 -newkey rsa:2048 \
          -keyout "$certdir/key.pem" -out "$certdir/cert.pem" -days 3650 -nodes \
          -subj "/CN=sunghyun-codesign" \
          -addext "keyUsage=critical,digitalSignature" \
          -addext "extendedKeyUsage=critical,codeSigning" \
          -addext "basicConstraints=critical,CA:false"
        /usr/bin/security import "$certdir/cert.pem" -k /Library/Keychains/System.keychain -T /usr/bin/codesign
        /usr/bin/security import "$certdir/key.pem" -k /Library/Keychains/System.keychain -T /usr/bin/codesign
        # -P (overwrite before unlink) is BSD-only, and activation runs with the
        # Nix system path ahead of /bin, where `rm` is GNU coreutils.
        /bin/rm -Pf "$certdir/key.pem"
        rm -f "$certdir/cert.pem"
        rmdir "$certdir"
      fi
      if /usr/bin/codesign --force --sign sunghyun-codesign --identifier com.anaclumos.sunghyun /usr/local/bin/sunghyun 2>/dev/null; then
        echo "sunghyun: signed with sunghyun-codesign (Accessibility grant survives rebuilds)"
      else
        echo "sunghyun: WARNING cert signing failed; ad-hoc signature (Accessibility resets on rebuild)"
      fi

      rm -f /usr/local/bin/sunghyun-os
      rm -f ${lib.escapeShellArg home}/.local/bin/sunghyun-os
      rm -f ${lib.escapeShellArg home}/.cargo/bin/sunghyun-os
    '';
  };
}
