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
        aegis-node = nixpkgs.lib.nixosSystem {
          inherit system;
          modules = [
            ./configuration.nix
            ./hardware-configuration.nix
            ./aegis-service.nix
            {
              # Inject our locally built ank-server package into the service module
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
              # Inject our locally built ank-server package into the service module
              services.aegis.package = self.packages.${system}.ank-server;
            }
          ];
        };
      };
    };
}