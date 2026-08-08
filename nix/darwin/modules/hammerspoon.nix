{ ... }:
{
  homebrew.casks = [ "hammerspoon" ];

  launchd.user.agents.hammerspoon = {
    serviceConfig = {
      Label = "com.anaclumos.hammerspoon";
      ProgramArguments = [ "/Applications/Hammerspoon.app/Contents/MacOS/Hammerspoon" ];
      RunAtLoad = true;
      KeepAlive.SuccessfulExit = false;
      ProcessType = "Interactive";
    };
  };
}
