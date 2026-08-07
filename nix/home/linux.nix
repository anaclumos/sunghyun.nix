# Linux-only Home Manager layer (standalone `homeConfigurations."sc@*-linux"`).
#
# Nothing here may be imported by the darwin host: macOS gets the same tools
# through nix-darwin `homebrew` (the vendor's own channel), and installing them
# twice would leave two binaries fighting over PATH order.
{ pkgs, ... }:
{
  home.packages = [
    # Cursor Agent CLI (binary: `cursor-agent`). macOS installs the official
    # `cursor-cli` Homebrew cask instead; nixpkgs is the portable path for
    # screenless Linux devices, which have no Homebrew and no GUI.
    pkgs.cursor-cli
  ];
}
