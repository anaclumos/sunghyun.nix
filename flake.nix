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
      # Primary host (LocalHostName / ComputerName: auracomputer)
      darwinConfigurations.auracomputer = mkHost "auracomputer" ./nix/darwin/hosts/auracomputer.nix;

      # Standalone Home Manager for non-NixOS Linux hosts (Ubuntu servers).
      # Portable layer only: no GUI, no darwin modules, headless-safe.
      homeConfigurations."sc@linux" = home-manager.lib.homeManagerConfiguration {
        pkgs = nixpkgs.legacyPackages.x86_64-linux;
        modules = [
          ./nix/home/portable.nix
          {
            home.username = "sc";
            home.homeDirectory = "/home/sc";
          }
        ];
      };

      packages.${system} = {
        default = self.darwinConfigurations.auracomputer.system;
        darwin-auracomputer = self.darwinConfigurations.auracomputer.system;
        # The Rust CLI residue (gates, verify, kanata safe-enable, key actions)
        # is built and shipped by the flake; install.sh never needs rustup/cargo.
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

      # After Nix is installed: nix flake check
      checks.${system}.darwin-eval = self.darwinConfigurations.auracomputer.system;

      formatter.${system} = nixpkgs.legacyPackages.${system}.nixfmt-rfc-style;

      # Convenience: nix run .#darwin-rebuild -- switch --flake .#auracomputer
      apps.${system}.darwin-rebuild = {
        type = "app";
        program = "${nix-darwin.packages.${system}.darwin-rebuild}/bin/darwin-rebuild";
      };
    };
}
