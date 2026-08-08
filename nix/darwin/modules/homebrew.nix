{ config, lib, ... }:
{
  assertions = [
    {
      assertion = lib.elem "karabiner-elements" (map (c: c.name) config.homebrew.casks);
      message = "homebrew.casks must keep karabiner-elements: onActivation.cleanup = \"uninstall\" would run its cask uninstall script, which deletes the shared Karabiner DriverKit VirtualHIDDevice files and bricks the keyboard. If it truly has to go, handle the DriverKit daemon migration by hand first.";
    }
  ];

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
      "mas"
      "mole"
      "pscale"
      "ripgrep"
      "rsync"
      "scc"
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
      "ghostty"
      "iina"
      "itsycal"
      "karabiner-elements"
      "keka"
      "linear"
      "macs-fan-control"
      "obsidian"
      "orbstack"
      "slack"
      "tailscale-app"
    ];
    masApps = {
      Amphetamine = 937984704;
      KakaoTalk = 869223134;
      "What Watt?" = 6756065687;
    };
  };
}
