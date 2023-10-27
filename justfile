# Show help
help:
  just --list --unsorted

# Run tests
test:
    cargo test

# Clean
clean:
    cargo clean
    rm -rf result/

# Build app
build-app:
    nix build
    mkdir -p target/
    rm -rf target/bin/
    mv result/* target/
    rm result

# Build server container image
build-server-image:
    nix build .#server-image
    mkdir -p target/
    mv result target/swandns.tar.gz

# Build client container image
build-client-image:
    nix build .#client-image
    mkdir -p target/
    mv result target/swandns-update.tar.gz

# Run server container
run-server-image: build-server-image
    podman run --rm --publish 8080:8080/tcp --publish 1053:1053/tcp --publish 1053:1053/udp docker-archive:./target/swandns.tar.gz

# Run client container
run-client-image: build-client-image
    podman run --rm docker-archive:./target/swandns-update.tar.gz

# Build all artifacts
build: build-app build-server-image build-client-image

# Build container images
build-images: build-server-image build-client-image

# Format Nix (.nix) files
format-nix:
    find ./ -name '*.nix' -exec nixfmt {} \; -exec nixfmt {} \;

# Format Rust (.rs) files
format-rs:
    cargo fmt --all -- --check

# Show outdated dependencies
outdated:
    cargo outdated --root-deps-only