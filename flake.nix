{
  description = ''
    sunghyun.nix: Nix front door for the owner's machines.
    macOS: darwin-rebuild switch --flake .#auracomputer
    Linux (non-NixOS, e.g. Ubuntu servers): home-manager switch --flake .#sc@linux
    `sunghyun` is the helper CLI for App Store / TCC / Accessibility / DriverKit
    UI gates and keyboard actions. Framework NixOS stays in anaclumos/nix.
  '';

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nix-darwin.url = "github:nix-darwin/nix-darwin/master";
    nix-darwin.inputs.nixpkgs.follows = "nixpkgs";
    home-manager.url = "github:nix-community/home-manager";
    home-manager.inputs.nixpkgs.follows = "nixpkgs";
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
      inherit (nix-darwin.lib) darwinSystem;

      mkHost =
        hostname: hostModule:
        darwinSystem {
          inherit system;
          specialArgs = {
            inherit inputs self;
            inherit hostname;
          };
          modules = [
            ./nix/darwin/modules/base.nix
            ./nix/darwin/modules/homebrew.nix
            ./nix/darwin/modules/kanata.nix
            ./nix/darwin/modules/defaults.nix
            ./nix/darwin/modules/hotkeys.nix
            ./nix/darwin/modules/sunghyun.nix
            hostModule
            home-manager.darwinModules.home-manager
            {
              home-manager.useGlobalPkgs = true;
              home-manager.useUserPackages = true;
              home-manager.backupFileExtension = "hm-backup";
              home-manager.extraSpecialArgs = {
                inherit inputs self;
              };
              home-manager.users.sc = import ./nix/darwin/modules/home.nix;
            }
          ];
        };
    in
    {
      darwinConfigurations.auracomputer = mkHost "auracomputer" ./nix/darwin/hosts/auracomputer.nix;
      darwinConfigurations.default = mkHost "default" ./nix/darwin/hosts/default.nix;

      # One output per Linux system, because a Home Manager configuration is
      # built for a fixed platform: a single `sc@linux` could not activate on an
      # aarch64 box. `sc@linux` stays as an x86_64 alias.
      homeConfigurations =
        let
          mkLinuxHome =
            linuxSystem:
            home-manager.lib.homeManagerConfiguration {
              pkgs = import nixpkgs {
                system = linuxSystem;
                # cursor-cli is unfree.
                config.allowUnfree = true;
              };
              modules = [
                ./nix/home/portable.nix
                ./nix/home/linux.nix
                {
                  home.username = "sc";
                  home.homeDirectory = "/home/sc";
                }
              ];
            };
        in
        {
          "sc@x86_64-linux" = mkLinuxHome "x86_64-linux";
          "sc@aarch64-linux" = mkLinuxHome "aarch64-linux";
          "sc@linux" = mkLinuxHome "x86_64-linux";
        };

      packages.${system} = {
        default = self.darwinConfigurations.auracomputer.system;
        darwin-auracomputer = self.darwinConfigurations.auracomputer.system;
        sunghyun = nixpkgs.legacyPackages.${system}.rustPlatform.buildRustPackage {
          pname = "sunghyun";
          version = "0.1.0";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          # Tests probe live TCC/launchctl surfaces; run them via cargo, not in
          # the Nix sandbox.
          doCheck = false;
          meta.mainProgram = "sunghyun";
        };
      };

      checks.${system} = {
        darwin-eval = self.darwinConfigurations.auracomputer.system;
        darwin-eval-default = self.darwinConfigurations.default.system;
      };

      formatter.${system} = nixpkgs.legacyPackages.${system}.nixfmt-rfc-style;

      apps.${system}.darwin-rebuild = {
        type = "app";
        program = "${nix-darwin.packages.${system}.darwin-rebuild}/bin/darwin-rebuild";
      };
    };
}
