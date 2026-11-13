#!/usr/bin/env bash
# Builds the Rust core as a static library for iOS and exposes it to Xcode from
# a directory that contains *only* the .a file.
#
# Why the separate directory: cargo emits libbussola.a and libbussola.dylib side
# by side for a crate with both staticlib and cdylib crates. With
# `LIBRARY_SEARCH_PATHS` pointing at that folder, `ld` picked the dylib and
# recorded the runner's absolute path as the install name, so the app crashed at
# launch with "Library not loaded: /Users/runner/.../libbussola.dylib".
# Linking against ios/RustFrameworks/libbussola.a cannot resolve to a dylib.
#
# Usage:
#   ./scripts/build-ios-rust.sh                 # device slice (aarch64-apple-ios)
#   INCLUDE_SIM=1 ./scripts/build-ios-rust.sh   # + simulator slices (xcframework)
#
# Requirements: macOS with Xcode (rustc needs the Apple linker).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="$ROOT/ios/RustFrameworks"

echo "==> rustup targets"
rustup target add aarch64-apple-ios
if [ "${INCLUDE_SIM:-0}" = "1" ]; then
  rustup target add aarch64-apple-ios-sim x86_64-apple-ios
fi

echo "==> cargo build (device)"
cargo build --release --target aarch64-apple-ios \
  --manifest-path "$ROOT/rust/Cargo.toml"

DEVICE_LIB="$ROOT/rust/target/aarch64-apple-ios/release/libbussola.a"
[ -f "$DEVICE_LIB" ] || { echo "static lib missing: $DEVICE_LIB" >&2; exit 1; }

# Fresh directory with the archive only: no .dylib can ever be picked up here.
rm -rf "$OUT"
mkdir -p "$OUT"
cp "$DEVICE_LIB" "$OUT/libbussola.a"

if [ "${INCLUDE_SIM:-0}" = "1" ]; then
  echo "==> cargo build (simulator)"
  cargo build --release --target aarch64-apple-ios-sim \
    --manifest-path "$ROOT/rust/Cargo.toml"
  cargo build --release --target x86_64-apple-ios \
    --manifest-path "$ROOT/rust/Cargo.toml"
  SIM_UNIVERSAL="$ROOT/rust/target/universal-ios-sim/release"
  mkdir -p "$SIM_UNIVERSAL"
  lipo -create \
    "$ROOT/rust/target/aarch64-apple-ios-sim/release/libbussola.a" \
    "$ROOT/rust/target/x86_64-apple-ios/release/libbussola.a" \
    -output "$SIM_UNIVERSAL/libbussola.a"
  echo "==> xcframework"
  rm -rf "$OUT/BussolaCore.xcframework"
  xcodebuild -create-xcframework \
    -library "$DEVICE_LIB" \
    -library "$SIM_UNIVERSAL/libbussola.a" \
    -output "$OUT/BussolaCore.xcframework"
fi

# Sanity check: an archive, not a shared object.
if file "$OUT/libbussola.a" | grep -q "shared library\|dynamically linked"; then
  echo "ERROR: $OUT/libbussola.a is a shared library, not a static archive" >&2
  exit 1
fi
ar -t "$OUT/libbussola.a" >/dev/null || {
  echo "ERROR: cannot list the archive contents" >&2
  exit 1
}
echo "==> $OUT/libbussola.a ($(du -h "$OUT/libbussola.a" | cut -f1), $(ar -t "$OUT/libbussola.a" | wc -l | tr -d ' ') object files)"
