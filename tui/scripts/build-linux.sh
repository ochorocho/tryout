#!/usr/bin/env bash
# Builds the static Linux binary for this machine's architecture into
# tryout/bin/ — what CI's DDEV-backed jobs install the add-on with (host and web
# container share the architecture there). For all three platforms on a Mac, see
# stage-bins.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
case "$(uname -m)" in
    x86_64|amd64)  arch=x86_64 ;;
    aarch64|arm64) arch=aarch64 ;;
    *) echo "no Linux build for $(uname -m)" >&2; exit 1 ;;
esac
target="${arch}-unknown-linux-musl"
command -v musl-gcc >/dev/null 2>&1 || sudo apt-get install -y musl-tools
rustup target add "${target}"
cargo build --release --locked --target "${target}"
mkdir -p ../tryout/bin
cp "target/${target}/release/tryout" "../tryout/bin/tryout-linux-${arch}"
grep -q '#ddev-generated' "../tryout/bin/tryout-linux-${arch}"
