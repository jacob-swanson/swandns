{
  description = "swandns";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/release-23.11";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nix, nixpkgs, ... }:
    let
      forAllSystems = function:
        nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ]
        (system: function nixpkgs.legacyPackages.${system});
    in {
      nixosModules = { default = ./nix/swandns.nix; };
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          name = "swandns";
          nativeBuildInputs = with pkgs; [
            just
            nixfmt
            statix
            protobuf
            cargo
            rustc
            jq
            skopeo
            cargo-outdated
          ];
          shellHook = ''
            ${let
              registriesConf = pkgs.writeText "registries.conf" ''
                [registries.search]
                registries = ['docker.io']
                [registries.block]
                registries = []
              '';
            in pkgs.writeScript "podman-setup" ''
              #!${pkgs.runtimeShell}
              # Dont overwrite customised configuration
              if ! test -f ~/.config/containers/policy.json; then
                install -Dm555 ${pkgs.skopeo.src}/default-policy.json ~/.config/containers/policy.json
              fi
              if ! test -f ~/.config/containers/registries.conf; then
                install -Dm555 ${registriesConf} ~/.config/containers/registries.conf
              fi
            ''}
          '';
        };
      });
      packages = forAllSystems (pkgs: {
        default = pkgs.callPackage ./. { inherit pkgs; };
        server-image = pkgs.dockerTools.buildLayeredImage {
          name = "swandns";
          config = {
            Env =
              [ "SWANDNS_CONFIG=/data/server.yaml" "SWANDNS_DATA_DIR=/data" ];
            Cmd = [
              "${pkgs.bash}/bin/bash"
              "-c"
              ''
                ${
                  pkgs.callPackage ./. { inherit pkgs; }
                }/bin/swandns --config "''${SWANDNS_CONFIG}"''
            ];
            Volumes = { "/data" = { }; };
          };
        };
        client-image = pkgs.dockerTools.buildLayeredImage {
          name = "swandns-update";
          config = {
            Env = [
              "SWANDNS_CONFIG=/data/client.yaml"
              "SWANDNS_SCHEDULE=*/5 * * * *"
            ];
            Cmd = [
              "${pkgs.bash}/bin/bash"
              "-c"
              ''
                ${
                  pkgs.callPackage ./. { inherit pkgs; }
                }/bin/swandns-update --config "''${SWANDNS_CONFIG}" --schedule "''${SWANDNS_SCHEDULE}"''
            ];
            Volumes = { "/data" = { }; };
          };
        };
      });
    };
}
