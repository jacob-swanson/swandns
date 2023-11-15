{ config, lib, pkgs, ... }:
with lib;
let
  swandns = pkgs.callPackage ../default.nix { };
  cfg = config.service.swandns-update;
  settingsFormat = pkgs.formats.yaml { };
  configFile = settingsFormat.generate "swandns-client.yaml" cfg.settings;
in {
  options.service.swandns-update = with lib.types; {
    enable = mkEnableOption "Enable Swandns Update service";

    user = mkOption {
      type = str;
      default = "swandns-update";
      description = "User account under which swandns-update runs";
    };

    group = mkOption {
      type = str;
      default = "swandns-update";
      description = "Group under which swandns-update runs";
    };

    settings = mkOption {
      type = submodule { freeformType = settingsFormat.type; };
      default = { };
    };

    package = mkOption {
      default = swandns;
      type = package;
      description = "swandns package to use.";
    };
  };

  config = mkIf cfg.enable {
    systemd.services.swandns-update = {
      description = "swandns-update";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      startLimitIntervalSec = 30;
      startLimitBurst = 5;
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${cfg.package}/bin/swandns-update --config ${configFile}";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    systemd.timers.swandns-update = {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnUnitActiveSec = "5m";
        Unit = "swandns-update.service";
      };
    };

    users.users = mkIf (cfg.user == "swandns-update") {
      swandns-update = {
        description = "Swandns Update Service";
        group = cfg.group;
        isSystemUser = true;
      };
    };

    users.groups =
      mkIf (cfg.group == "swandns-update") { swandns-update = { }; };
  };
}
