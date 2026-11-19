# Sourced by the container build scripts: installs the pinned Rust toolchain
# and unpacks the source tarball into /build/poe-overlay-$VERSION.
curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --default-toolchain "$RUST_VERSION"
. "$HOME/.cargo/env"
mkdir -p /build
tar -xzf "/dist/poe-overlay-$VERSION.tar.gz" -C /build
cd "/build/poe-overlay-$VERSION"
# Release binaries without their symbol table.
export CARGO_PROFILE_RELEASE_STRIP=symbols
