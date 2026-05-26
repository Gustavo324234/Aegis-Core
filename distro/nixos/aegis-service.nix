{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.aegis;
in {
  options.services.aegis = {
    enable = mkEnableOption "Aegis Cognitive OS Server";

    package = mkOption {
      type = types.package;
      default = pkgs.ank-server;
      description = "The ank-server package to run.";
    };

    dataDir = mkOption {
      type = types.path;
      default = "/var/lib/aegis";
      description = "Directory to store Aegis persistent state databases, logs, plugins.";
    };

    configDir = mkOption {
      type = types.path;
      default = "/etc/aegis";
      description = "Directory for Aegis environment configurations and keys.";
    };

    port = mkOption {
      type = types.port;
      default = 8000;
      description = "Port to listen on for HTTP.";
    };
  };

  config = mkIf cfg.enable {
    users.users.aegis = {
      isSystemUser = true;
      group = "aegis";
      description = "Aegis OS system user";
      home = cfg.dataDir;
    };
    users.groups.aegis = {};

    systemd.services.aegis = {
      description = "Aegis OS — Cognitive Operating System";
      after = [ "network.target" "local-fs.target" ];
      wants = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        Type = "simple";
        User = "aegis";
        Group = "aegis";
        EnvironmentFile = "${cfg.configDir}/aegis.env";
        Environment = [
          "RUST_LOG=info"
          "AEGIS_DATA_DIR=${cfg.dataDir}"
          "AEGIS_HTTP_PORT=${toString cfg.port}"
          "ANK_HTTP_PORT=${toString cfg.port}"
        ];
        ExecStart = "${cfg.package}/bin/ank-server";
        Restart = "on-failure";
        RestartSec = "5s";
        TimeoutStopSec = "10s";

        # SRE Hardening
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ReadWritePaths = [ cfg.dataDir cfg.configDir ];
        PrivateTmp = true;
        ProtectKernelTunables = true;
        ProtectControlGroups = true;
        RestrictRealtime = true;
      };
    };
  };
}