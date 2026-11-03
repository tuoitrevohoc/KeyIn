#!/bin/sh
# Build the Rust engine for iOS devices and the simulator.
# ENGINE_DIR defaults to the repository root; point it at a clean checkout if needed.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
engine="${ENGINE_DIR:-$here/../..}"
out="$here/lib"
export IPHONEOS_DEPLOYMENT_TARGET=16.0
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    rustup target add "$target" >/dev/null
    cargo build --manifest-path "$engine/Cargo.toml" --release --target "$target"
    mkdir -p "$out/$target"
    cp "$engine/target/$target/release/libkeyin.a" "$out/$target/"
done
