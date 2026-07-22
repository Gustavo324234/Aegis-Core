{
  description = "Aegis OS Distro - Minimal, Immutable Cognitive OS Linux Base";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs, ... }@inputs:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in {
      packages.${system} = {
        ank-server = pkgs.callPackage ./ank-server.nix {};
        default = self.packages.${system}.ank-server;
      };

      nixosConfigurations = {
        aegis-server = nixpkgs.lib.nixosSystem {
          inherit system;
          modules = [
            ./configuration.nix
            ./hardware-configuration.nix
            ./aegis-service.nix
            ./profile-server.nix
            {
              services.aegis.package = self.packages.${system}.ank-server;
            }
          ];
        };

        aegis-kiosk = nixpkgs.lib.nixosSystem {
          inherit system;
          modules = [
            ./configuration.nix
            ./hardware-configuration.nix
            ./aegis-service.nix
            ./profile-kiosk.nix
            {
              services.aegis.package = self.packages.${system}.ank-server;
            }
          ];
        };

        aegis-iso = nixpkgs.lib.nixosSystem {
          inherit system;
          modules = [
            "${nixpkgs}/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix"
            ./configuration.nix
            ./aegis-service.nix
            ./profile-server.nix
            {
              services.aegis.package = self.packages.${system}.ank-server;
            }
          ];
        };
      };
    };
}