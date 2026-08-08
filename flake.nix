{
  description = "Declarative Nix front door for the owner's machines: nix-darwin on macOS, portable Home Manager on Linux";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nix-darwin.url = "github:nix-darwin/nix-darwin/master";
    nix-darwin.inputs.nixpkgs.follows = "nixpkgs";
    home-manager.url = "github:nix-community/home-manager";
    home-manager.inputs.nixpkgs.follows = "nixpkgs";
    sunghyun-sans.url = "github:anaclumos/sunghyun-sans";
    sunghyun-sans.inputs.nixpkgs.follows = "nixpkgs";
    tokenmaxxing.url = "github:anaclumos/tokenmaxxing";
    tokenmaxxing.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      nix-darwin,
      home-manager,
      ...
    }:
    let
      system = "aarch64-darwin";

      mkHost =
        hostModule:
        nix-darwin.lib.darwinSystem {
          inherit system;
          specialArgs = {
            inherit inputs;
          };
          modules = [
            ./nix/darwin/modules/agents.nix
            ./nix/darwin/modules/base.nix
            ./nix/darwin/modules/homebrew.nix
            ./nix/darwin/modules/defaults.nix
            ./nix/darwin/modules/hammerspoon.nix
            inputs.tokenmaxxing.darwinModules.withOverlay
            { programs.tokenmaxxing.enable = true; }
            hostModule
            home-manager.darwinModules.home-manager
            {
              home-manager.useGlobalPkgs = true;
              home-manager.useUserPackages = true;
              home-manager.backupFileExtension = "hm-backup";
              home-manager.users.sc = import ./nix/darwin/modules/home.nix;
            }
          ];
        };

      mkLinuxHome =
        linuxSystem:
        home-manager.lib.homeManagerConfiguration {
          pkgs = import nixpkgs {
            system = linuxSystem;
            config.allowUnfree = true;
          };
          modules = [
            ./nix/home/portable.nix
            ./nix/home/linux.nix
            ./nix/home/fonts.nix
            {
              home.username = "sc";
              home.homeDirectory = "/home/sc";
            }
          ];
          extraSpecialArgs = {
            inherit inputs;
          };
        };
    in
    {
      darwinConfigurations.auracomputer = mkHost ./nix/darwin/hosts/auracomputer.nix;
      darwinConfigurations.default = mkHost ./nix/darwin/hosts/default.nix;

      homeConfigurations = {
        "sc@x86_64-linux" = mkLinuxHome "x86_64-linux";
        "sc@aarch64-linux" = mkLinuxHome "aarch64-linux";
        "sc@linux" = mkLinuxHome "x86_64-linux";
      };

      checks.${system} = {
        darwin-eval = self.darwinConfigurations.auracomputer.system;
        darwin-eval-default = self.darwinConfigurations.default.system;
      };

      formatter.${system} = nixpkgs.legacyPackages.${system}.nixfmt;
    };
}
