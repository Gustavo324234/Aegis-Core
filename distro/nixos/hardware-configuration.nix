{ config, lib, pkgs, modulesPath, ... }:

{
  imports = [ (modulesPath + "/profiles/qemu-guest.nix") ];

  boot.initrd.availableKernelModules = [ "ahci" "xhci_pci" "virtio_pci" "virtio_scsi" "sd_mod" "sr_mod" ];
  boot.initrd.kernelModules = [ "dm-snapshot" "dm-crypt" ];
  boot.kernelModules = [ "kvm-intel" "kvm-amd" ];
  boot.extraModulePackages = [ ];

  # Immutable partition layout:
  # 1. Boot/EFI: /boot (FAT32, read-write but system static)
  # 2. Root system: / (Read-only ext4 partition)
  # 3. Data partition: Encrypted LUKS volume, decrypted as "crypted-data" and mounted as /data (Read-Write)

  fileSystems."/" =
    { device = "/dev/disk/by-label/AEGIS_ROOT";
      fsType = "ext4";
      options = [ "ro" "noatime" "noload" ];
    };

  fileSystems."/boot" =
    { device = "/dev/disk/by-label/AEGIS_BOOT";
      fsType = "vfat";
    };

  # Encrypted state partition
  boot.initrd.luks.devices."crypted-data" = {
    device = "/dev/disk/by-label/AEGIS_CRYPTDATA";
    preLVM = true;
    allowDiscards = true;
  };

  fileSystems."/data" =
    { device = "/dev/mapper/crypted-data";
      fsType = "ext4";
      options = [ "rw" "noatime" "nodiratime" ];
    };

  # Swap file on the encrypted /data partition prevents leaking key material to disk blocks
  swapDevices = [ { device = "/data/swapfile"; size = 4096; } ];

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
}