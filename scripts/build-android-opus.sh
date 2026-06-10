#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

OUTPUT_DIR="${PROJECT_ROOT}/dist/android-opus"
ANDROID_API="${ANDROID_API:-21}"
NDK_ROOT="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
BUILD_ABIS="arm64-v8a armeabi-v7a"
JOBS="${JOBS:-$(nproc 2>/dev/null || echo 4)}"
OPUS_VERSION="${OPUS_VERSION:-1.5.2}"

usage() {
    cat <<'EOF'
Usage:
  scripts/build-android-opus.sh [options]

Options:
  --output <dir>          Output root. Default: dist/android-opus
  --ndk <dir>             Android NDK root. Defaults to ANDROID_NDK_HOME or ANDROID_NDK_ROOT.
  --api <level>           Android API level. Default: 21.
  --abis <list>           Space/comma separated ABI list. Default: arm64-v8a armeabi-v7a.
  -h, --help              Show this help.

The output layout is compatible with ONE_KVM_ANDROID_OPUS_ROOT:
  <output>/arm64-v8a/include/opus/opus.h
  <output>/arm64-v8a/lib/libopus.so
  <output>/armeabi-v7a/include/opus/opus.h
  <output>/armeabi-v7a/lib/libopus.so
EOF
}

fail() {
    echo "Error: $*" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
    --output)
        OUTPUT_DIR="${2:-}"
        shift 2
        ;;
    --ndk)
        NDK_ROOT="${2:-}"
        shift 2
        ;;
    --api)
        ANDROID_API="${2:-}"
        shift 2
        ;;
    --abis)
        BUILD_ABIS="${2:-}"
        shift 2
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        fail "Unknown argument: $1"
        ;;
    esac
done

[[ -n "$NDK_ROOT" ]] || fail "--ndk or ANDROID_NDK_HOME/ANDROID_NDK_ROOT is required"
[[ -d "$NDK_ROOT/toolchains/llvm/prebuilt" ]] || fail "Invalid NDK root: $NDK_ROOT"

SOURCE_DIR="${PROJECT_ROOT}/.tmp/android-opus-src"
rm -rf "$SOURCE_DIR"
mkdir -p "$SOURCE_DIR"
tarball="${PROJECT_ROOT}/.tmp/opus-${OPUS_VERSION}.tar.gz"
url="https://downloads.xiph.org/releases/opus/opus-${OPUS_VERSION}.tar.gz"
curl -fsSL "$url" -o "$tarball"
tar -xzf "$tarball" -C "$SOURCE_DIR" --strip-components=1

SOURCE_DIR="$(cd "$SOURCE_DIR" && pwd)"
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
TOOLCHAIN="${NDK_ROOT}/toolchains/llvm/prebuilt/${HOST_TAG}"

normalize_abis() {
    printf '%s\n' "$BUILD_ABIS" | tr ',' ' '
}

build_one() {
    local abi="$1"
    local prefix build_dir

    case "$abi" in
    arm64-v8a | armeabi-v7a) ;;
    *) fail "Unsupported ABI: $abi" ;;
    esac

    prefix="${OUTPUT_DIR}/${abi}"
    build_dir="${PROJECT_ROOT}/.tmp/opus-android-build/${abi}"

    rm -rf "$build_dir"
    mkdir -p "$build_dir" "$prefix"

    (
        cd "$build_dir"
        case "$abi" in
        arm64-v8a)
            export CC="${TOOLCHAIN}/bin/aarch64-linux-android${ANDROID_API}-clang"
            export CXX="${TOOLCHAIN}/bin/aarch64-linux-android${ANDROID_API}-clang++"
            export HOST_TRIPLE="aarch64-linux-android"
            ;;
        armeabi-v7a)
            export CC="${TOOLCHAIN}/bin/armv7a-linux-androideabi${ANDROID_API}-clang"
            export CXX="${TOOLCHAIN}/bin/armv7a-linux-androideabi${ANDROID_API}-clang++"
            export HOST_TRIPLE="arm-linux-androideabi"
            ;;
        esac
        export AR="${TOOLCHAIN}/bin/llvm-ar"
        export RANLIB="${TOOLCHAIN}/bin/llvm-ranlib"
        export STRIP="${TOOLCHAIN}/bin/llvm-strip"
        export CFLAGS="-fPIC"
        export CXXFLAGS="-fPIC"
        export LDFLAGS=""
        "$SOURCE_DIR/configure" \
            --prefix="$prefix" \
            --host="$HOST_TRIPLE" \
            --disable-static \
            --enable-shared \
            --disable-doc \
            --disable-extra-programs \
            --with-pic
        make -j"$JOBS"
        make install
    )

    echo "Built Opus for ${abi}: ${prefix}"
}

for abi in $(normalize_abis); do
    build_one "$abi"
done

cat <<EOF

Done.

Use this when building the Android APK:
  export ONE_KVM_ANDROID_OPUS_ROOT="${OUTPUT_DIR}"
  cd android && ./gradlew :app:assembleDebug
EOF
