# sunghyun CLI wiring: the flake builds the Rust binary; activation keeps a
# stable copy at /usr/local/bin so TCC grants (Accessibility for tiling)
# survive nix store path changes across rebuilds. Grants are still per-CDHash
# for adhoc-signed binaries, so a content change drops them; `sunghyun
# post-switch` re-converges via open-Settings-pane + poll.
#
# Activation must never hang on App Store / TCC waits (no TTY under root
# activation) and must never instruct the human to run follow-up commands.
# Residual GUI gates run via `sunghyun post-switch` (install.sh drives it).
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
      # Migration (2026-08-08): binary renamed from sunghyun-os; remove every
      # stale copy so no orphan can shadow `sunghyun` via PATH order. The
      # user-dir copies came from historical `cargo install` runs and were
      # found live after the first cleanup only covered /usr/local/bin.
      rm -f /usr/local/bin/sunghyun-os
      rm -f ${lib.escapeShellArg home}/.local/bin/sunghyun-os
      rm -f ${lib.escapeShellArg home}/.cargo/bin/sunghyun-os
    '';
  };
}
