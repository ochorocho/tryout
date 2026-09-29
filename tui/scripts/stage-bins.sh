#!/usr/bin/env bash
# Builds the three release binaries into tryout/bin/, where the launcher
# (tryout/tryout) looks for them — what the release workflow commits, for a local
# `ddev add-on get <this checkout>`. macOS only: it needs lipo, plus
# cargo-zigbuild for the static Linux builds.
set -euo pipefail

cd "$(dirname "$0")/.."
out="../tryout/bin"
mkdir -p "${out}"

for t in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --release --locked --target "${t}"
done
# Written next to the old file and moved over it: overwriting a binary in place
# gets it killed by macOS on its next start (its code signature is cached per file).
lipo -create -output "${out}/tryout-macos-universal.new" \
    target/aarch64-apple-darwin/release/tryout target/x86_64-apple-darwin/release/tryout
mv -f "${out}/tryout-macos-universal.new" "${out}/tryout-macos-universal"

for pair in x86_64-unknown-linux-musl:x86_64 aarch64-unknown-linux-musl:aarch64; do
    t="${pair%%:*}"
    cargo zigbuild --release --locked --target "${t}"
    cp "target/${t}/release/tryout" "${out}/tryout-linux-${pair##*:}.new"
    mv -f "${out}/tryout-linux-${pair##*:}.new" "${out}/tryout-linux-${pair##*:}"
done

# DDEV only manages (updates, removes) a file that carries its marker.
for f in "${out}"/tryout-*; do
    grep -q '#ddev-generated' "${f}" || { echo "no #ddev-generated in ${f}" >&2; exit 1; }
done
ls -l "${out}"
