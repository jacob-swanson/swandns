{ pkgs ? import <nixpkgs> { } }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "swandns";
  version = "1.0.0";
  cargoLock.lockFile = ./Cargo.lock;
  src = ./.;
  nativeBuildInputs = [ pkgs.protobuf ];
  doCheck = false;
}
