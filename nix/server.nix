{ config, lib, pkgs, ... }:
with lib;
let
  swandns = pkgs.callPackage ../default.nix { };
  cfg = config.services.swandns;
  settingsFormat = pkgs.formats.yaml { };
  configFile = settingsFormat.generate "swandns-server.yaml"
    (cfg.settings // { data_dir = cfg.dataDir; });
in {
  options.services.swandns = with lib.types; {
    enable = mkEnableOption "Enable Swandns service";

    user = mkOption {
      type = str;
      default = "swandns";
      description = "User account under which swandns runs";
    };

    group = mkOption {
      type = str;
      default = "nogroup";
      description = "Group under which swandns runs";
    };

    settings = mkOption {
      type = submodule { freeformType = settingsFormat.type; };
      default = { };
    };

    dataDir = mkOption {
      type = path;
      default = "/var/lib/swandns";
    };

    package = mkOption {
      default = swandns;
      type = package;
      description = "swandns package to use.";
    };
  };

  config = mkIf cfg.enable {
    systemd.services.swandns = {
      description = "swandns";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      startLimitIntervalSec = 30;
      startLimitBurst = 5;
      serviceConfig = {
        Type = "simple";
        User = cfg.user;
        Group = cfg.group;
        WorkingDirectory = cfg.dataDir;
        ExecStart = "${cfg.package}/bin/swandns --config ${configFile}";
        Restart = "on-failure";
        RestartSec = 5;
        AmbientCapabilities = "CAP_NET_BIND_SERVICE";
      };
    };

    users.users = mkIf (cfg.user == "swandns") {
      swandns = {
        description = "Swandns Service";
        home = cfg.dataDir;
        group = cfg.group;
        isSystemUser = true;
      };
    };
  };
}
