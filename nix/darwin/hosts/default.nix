# Deliberately sets no ComputerName / LocalHostName / HostName: nix-darwin
# leaves all three alone when they are null, so an unnamed Mac keeps the
# identity Setup Assistant gave it. A VM that activated a named host once came
# up as auracomputer-2.local beside the real Mac.
{ ... }:
{
}
