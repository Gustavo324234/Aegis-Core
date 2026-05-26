{ config, pkgs, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ./aegis-service.nix
  ];

  # Boot loader config
  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = true;

  networking.hostName = "aegis-citadel";
  networking.networkmanager.enable = true;

  # Set your time zone.
  time.timeZone = "UTC";

  # Select internationalisation properties.
  i18n.defaultLocale = "en_US.UTF-8";

  # Minimise packages for small footprint and minimal attack surface
  environment.systemPackages = with pkgs; [
    curl
    gitMinimal
    openssl
    sqlite
    cryptsetup
    htop
  ];

  # Enable SSH for secure remote SRE access, using only key-based authentication
  services.openssh = {
    enable = true;
    settings = {
      PasswordAuthentication = false;
      KbdInteractiveAuthentication = false;
      PermitRootLogin = "no";
    };
  };

  # Enable the Aegis Cognitive OS Service!
  services.aegis = {
    enable = true;
    port = 8000;
  };

  # Declaratively lock down write permissions by binding mutable paths to /data
  system.activationScripts.aegis-dirs = {
    text = ''
      mkdir -p /data/etc/aegis
      mkdir -p /data/var/lib/aegis
      mkdir -p /data/var/lib/aegis/plugins
      mkdir -p /data/var/lib/aegis/logs
      mkdir -p /data/var/lib/aegis/users

      # Ensure ownership matches the aegis service user
      chown -R aegis:aegis /data/etc/aegis
      chown -R aegis:aegis /data/var/lib/aegis
      chmod 700 /data/etc/aegis
      chmod 750 /data/var/lib/aegis
    '';
  };

  fileSystems."/etc/aegis" = {
    device = "/data/etc/aegis";
    options = [ "bind" ];
  };

  fileSystems."/var/lib/aegis" = {
    device = "/data/var/lib/aegis";
    options = [ "bind" ];
  };

  # Firewall configurations
  networking.firewall = {
    enable = true;
    allowedTCPPorts = [ 80 443 8000 50051 22 ]; # HTTP, HTTPS, Web UI, gRPC, SSH
  };

  # Automatic garbage collection to keep system footprint low
  nix.gc = {
    automatic = true;
    dates = "weekly";
    options = "--delete-older-than 30d";
  };

  # System state version
  system.stateVersion = "24.05";
}