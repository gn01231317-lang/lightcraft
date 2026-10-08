#!/usr/bin/env bash
# Build the Android app:  $DIST/lightcraft-android-<version>.apk
#
# Usage: packaging/android/build.sh [--fast] [--abi arm64-v8a|x86_64|all]
#   --fast   the `release` Cargo profile instead of `dist` (no LTO: quicker to build, a bit slower)
#   --abi    which Android ABI(s) to build (default arm64-v8a, i.e. phones; x86_64 is the emulator)
#
# Needs (see docs/android.md): the rustup targets for the ABIs (aarch64-linux-android,
# x86_64-linux-android); an Android SDK (ANDROID_HOME) with platform 35; the NDK (ANDROID_NDK_HOME,
# r26 or later; only its linker is used, everything compiled is Rust); Java 17 and Gradle 8.9+ on
# PATH. The APK is signed with the Gradle debug key unless LIGHTCRAFT_ANDROID_KEYSTORE (and
# …_KEYSTORE_PASSWORD, …_KEY_ALIAS, …_KEY_PASSWORD) name a release key.
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/android"

PROFILE=dist
ABIS=(arm64-v8a)
while [ $# -gt 0 ]; do
  case "$1" in
    --fast) PROFILE=release ;;
    --abi)
      shift
      case "${1:-}" in
        all) ABIS=(arm64-v8a x86_64) ;;
        arm64-v8a | x86_64) ABIS=("$1") ;;
        *) echo "error: --abi arm64-v8a, x86_64 or all" >&2; exit 2 ;;
      esac
      ;;
    *) echo "error: unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

NDK="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
[ -n "$NDK" ] && [ -d "$NDK" ] || { echo "error: set ANDROID_NDK_HOME to the Android NDK" >&2; exit 1; }
case "$(uname -s)" in
  Linux) HOST=linux-x86_64 ;;
  Darwin) HOST=darwin-x86_64 ;; # the NDK's macOS toolchain is universal under this name
  *) echo "error: build on Linux or macOS" >&2; exit 1 ;;
esac
BIN="$NDK/toolchains/llvm/prebuilt/$HOST/bin"
# the manifest's minSdk
API=29

GEN="$CARGO_TARGET_DIR/android"
rm -rf "$GEN"
mkdir -p "$GEN/jniLibs" "$GEN/res"

for abi in "${ABIS[@]}"; do
  case "$abi" in
    arm64-v8a) triple=aarch64-linux-android ;;
    x86_64) triple=x86_64-linux-android ;;
  esac
  linker="$BIN/$triple$API-clang"
  [ -x "$linker" ] || { echo "error: $linker not found (NDK too old?)" >&2; exit 1; }
  var="$(echo "$triple" | tr 'a-z-' 'A-Z_')"
  export "CARGO_TARGET_${var}_LINKER=$linker"
  # 16 KB pages (Android 15+ devices) need 16 KB-aligned segments
  export "CARGO_TARGET_${var}_RUSTFLAGS=-C link-arg=-Wl,-z,max-page-size=16384"
  (cd "$ROOT" && cargo build -p lightcraft-android-main --lib --target "$triple" --profile "$PROFILE" --config "profile.$PROFILE.strip=true")
  mkdir -p "$GEN/jniLibs/$abi"
  cp "$CARGO_TARGET_DIR/$triple/$PROFILE/libmain.so" "$GEN/jniLibs/$abi/"
done

# launcher icon from assets/app-icon (Android scales each to its density)
ICONS="$ROOT/assets/app-icon/hicolor"
for d in mdpi:48 hdpi:64 xhdpi:128 xxhdpi:256 xxxhdpi:256; do
  mkdir -p "$GEN/res/mipmap-${d%%:*}"
  cp "$ICONS/${d##*:}x${d##*:}/apps/ai.storyteller.lightcraft.png" "$GEN/res/mipmap-${d%%:*}/ic_launcher.png"
done

gradle -p "$HERE" --no-daemon assembleRelease -PlightcraftVersion="$VERSION" -PlightcraftGenerated="$GEN"

mkdir -p "$DIST"
OUT="$DIST/lightcraft-android-$VERSION.apk"
cp "$HERE/app/build/outputs/apk/release/app-release.apk" "$OUT"
echo "wrote $OUT"
ls -lh "$OUT"
