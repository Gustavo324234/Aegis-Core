{ config, pkgs, ... }:

{
  # 1. Habilitar la aceleración gráfica por hardware (OpenGL / Mesa)
  hardware.opengl = {
    enable = true;
    driSupport = true;
    driSupport32Bit = true;
  };

  # 2. Configurar el servidor de audio PipeWire para WebRTC / Siren
  security.rtkit.enable = true;
  services.pipewire = {
    enable = true;
    alsa.enable = true;
    alsa.support32Bit = true;
    pulse.enable = true;
  };

  # 3. Crear usuario dedicado para el quiosco
  users.users.aegis-kiosk = {
    isNormalUser = true;
    extraGroups = [ "video" "audio" "sound" "input" ];
    description = "Aegis OS Kiosk UI User";
    initialPassword = "kiosk";
  };

  # 4. Habilitar el compositor Wayland Cage en modo quiosco ejecutando Chromium
  services.cage = {
    enable = true;
    user = "aegis-kiosk";
    # Lanzar Chromium a pantalla completa sin advertencias y sobre Wayland nativo
    program = "${pkgs.chromium}/bin/chromium --kiosk --no-first-run --simulate-outdated-no-btn --no-default-browser-check --ozone-platform=wayland --enable-features=UseOzonePlatform --app=http://localhost:8000";
  };

  # 5. Evitar que la consola interactiva se apague por inactividad
  systemd.targets.sleep.enable = false;
  systemd.targets.suspend.enable = false;
  systemd.targets.hibernate.enable = false;
  systemd.targets.hybrid-sleep.enable = false;
}
