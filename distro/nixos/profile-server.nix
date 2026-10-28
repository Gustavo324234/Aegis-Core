{ config, lib, pkgs, ... }:

{
  # 1. Modo Headless puro: desactivar subsistemas gráficos y sonido innecesario en servidor
  services.xserver.enable = false;
  sound.enable = lib.mkDefault false;
  hardware.pulseaudio.enable = false;

  # 2. Optimización para Laptop como Home Server (Clamshell / Tapa Cerrada):
  # Evitar que el cierre de la tapa suspenda el procesador o interrumpa el bucle de agentes 24/7
  services.logind = {
    lidSwitch = "ignore";
    lidSwitchExternalPower = "ignore";
    lidSwitchDocked = "ignore";
    extraConfig = ''
      HandlePowerKey=poweroff
      IdleAction=ignore
    '';
  };

  # 3. Desactivar targets de suspensión automática de systemd para operación continua 24/7
  systemd.targets.sleep.enable = false;
  systemd.targets.suspend.enable = false;
  systemd.targets.hibernate.enable = false;
  systemd.targets.hybrid-sleep.enable = false;

  # 4. Gestión térmica y de energía eficiente en CPU
  powerManagement = {
    enable = true;
    cpuFreqGovernor = lib.mkDefault "schedutil";
  };

  # 5. Parámetros de red y descriptores de archivo orientados a servidor concurrente (Axum + gRPC)
  boot.kernel.sysctl = {
    "fs.file-max" = 2097152;
    "net.core.somaxconn" = 4096;
    "net.ipv4.tcp_keepalive_time" = 60;
    "net.ipv4.tcp_keepalive_intvl" = 10;
    "net.ipv4.tcp_keepalive_probes" = 6;
  };
}
