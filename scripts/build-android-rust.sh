#!/usr/bin/env bash
# Builds the Rust core for Android and copies the .so files into the Gradle
# project (app/src/main/jniLibs/<abi>/libbussola.so).
#
# Usage:
#   ANDROID_NDK_HOME=/path/to/ndk ./scripts/build-android-rust.sh
#   INCLUDE_X86=1 ...        # also build the x86_64 slice (emulators)
#
# Requirements: rustup with the android targets, an NDK (r26+).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to your NDK directory}"

API="${ANDROID_API:-24}"
case "$(uname -s)" in
  Linux)  HOST_TAG=linux-x86_64 ;;
  Darwin) HOST_TAG=darwin-x86_64 ;;
  *) echo "unsupported host: $(uname -s)" >&2; exit 1 ;;
esac
TC="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG/bin"
[ -x "$TC/aarch64-linux-android${API}-clang" ] || {
  echo "clang not found in $TC (wrong ANDROID_NDK_HOME?)" >&2
  exit 1
}

TARGETS=("aarch64-linux-android:arm64-v8a" "armv7-linux-androideabi:armeabi-v7a")
if [ "${INCLUDE_X86:-0}" = "1" ]; then
  TARGETS+=("x86_64-linux-android:x86_64")
fi

# Android 15 (API 35) requires 16 KB page-size compatible shared libraries on
# arm64. Harmless on the other ABIs.
SEGMENT_FLAGS="-C link-arg=-Wl,-z,max-page-size=16384"

for entry in "${TARGETS[@]}"; do
  target="${entry%%:*}"
  abi="${entry##*:}"

  case "$target" in
    aarch64-linux-android)      linker="$TC/aarch64-linux-android${API}-clang" ;;
    armv7-linux-androideabi)    linker="$TC/armv7a-linux-androideabi${API}-clang" ;;
    x86_64-linux-android)       linker="$TC/x86_64-linux-android${API}-clang" ;;
  esac

  # Cargo reads CARGO_TARGET_<TRIPLE_UPPER_UNDERSCORE>_LINKER.
  env_var="CARGO_TARGET_$(echo "$target" | tr 'a-z-' 'A-Z_')_LINKER"
  echo "==> $target ($abi)"
  env "$env_var=$linker" \
      CARGO_TARGET_DIR="$ROOT/rust/target" \
      RUSTFLAGS="$SEGMENT_FLAGS" \
      cargo build --release --target "$target" \
        --config 'profile.release.strip="symbols"' \
        --manifest-path "$ROOT/rust/Cargo.toml"

  out="$ROOT/android/app/src/main/jniLibs/$abi"
  mkdir -p "$out"
  cp "$ROOT/rust/target/$target/release/libbussola.so" "$out/libbussola.so"
  echo "    -> $out/libbussola.so ($(du -h "$out/libbussola.so" | cut -f1))"
done

echo
echo "jniLibs ready:"
du -sh "$ROOT"/android/app/src/main/jniLibs/* 2>/dev/null || true
