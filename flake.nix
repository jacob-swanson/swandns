{
  description = "swandns";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/release-23.05";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nix, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        swandns = pkgs.callPackage ./. { inherit pkgs; };
        podmanSetupScript = let
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
        '';
      in with pkgs; rec {
        # Development environment
        devShell = mkShell {
          name = "swandns";
          nativeBuildInputs = [
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
            ${podmanSetupScript}
          '';
        };

        packages = {
          default = swandns;
          server-image = pkgs.dockerTools.buildLayeredImage {
            name = "swandns";
            config = {
              Env =
                [ "SWANDNS_CONFIG=/data/server.yaml" "SWANDNS_DATA_DIR=/data" ];
              Cmd = [
                "${pkgs.bash}/bin/bash"
                "-c"
                ''${swandns}/bin/swandns --config "''${SWANDNS_CONFIG}"''
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
                  ${swandns}/bin/swandns-update --config "''${SWANDNS_CONFIG}" --schedule "''${SWANDNS_SCHEDULE}"''
              ];
              Volumes = { "/data" = { }; };
            };
          };
        };
      });
}
