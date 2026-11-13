#!/usr/bin/env bash
# Builds the Rust core as a static library for iOS (device only) so the Xcode
# project can link it with `-lbussola`.
#
# Usage:
#   ./scripts/build-ios-rust.sh            # device slice (aarch64-apple-ios)
#   INCLUDE_SIM=1 ./scripts/build-ios-rust.sh   # + simulator slices
#
# Requirements: macOS with Xcode (rustc needs the Apple linker).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)

echo "==> rustup targets"
rustup target add aarch64-apple-ios
if [ "${INCLUDE_SIM:-0}" = "1" ]; then
  rustup target add aarch64-apple-ios-sim x86_64-apple-ios
fi

echo "==> cargo build (device)"
cargo build --release --target aarch64-apple-ios \
  --manifest-path "$ROOT/rust/Cargo.toml"

if [ "${INCLUDE_SIM:-0}" = "1" ]; then
  cargo build --release --target aarch64-apple-ios-sim \
    --manifest-path "$ROOT/rust/Cargo.toml"
  cargo build --release --target x86_64-apple-ios \
    --manifest-path "$ROOT/rust/Cargo.toml"
  mkdir -p "$ROOT/rust/target/universal-ios-sim/release"
  lipo -create \
    "$ROOT/rust/target/aarch64-apple-ios-sim/release/libbussola.a" \
    "$ROOT/rust/target/x86_64-apple-ios/release/libbussola.a" \
    -output "$ROOT/rust/target/universal-ios-sim/release/libbussola.a"
  echo "    -> universal simulator lib ready"
fi

LIB="$ROOT/rust/target/aarch64-apple-ios/release/libbussola.a"
[ -f "$LIB" ] || { echo "static lib missing: $LIB" >&2; exit 1; }
echo "==> $LIB ($(du -h "$LIB" | cut -f1))"
