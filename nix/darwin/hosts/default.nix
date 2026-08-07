# Generic host: any Mac that this flake has no named config for.
#
# Deliberately empty of identity. `nix/darwin/hosts/auracomputer.nix` names the
# owner's primary Mac and is the ONLY config allowed to set ComputerName /
# LocalHostName / HostName. Before 2026-08-08 the fallback was
# `.#auracomputer`, so a brand-new machine renamed itself to auracomputer on
# first activation; in a VM on the same LAN as the real Mac the guest came up
# as `auracomputer-2.local`. A machine's own identity is not ours to take:
# nix-darwin leaves all three names untouched when the options are null
# (their default), so this config keeps whatever Setup Assistant chose.
{ ... }:
{
  # No networking.hostName / localHostName / computerName here. On purpose.
}
