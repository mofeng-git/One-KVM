#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

SOURCE_DIR=""
OUTPUT_DIR="${PROJECT_ROOT}/dist/android-opus"
ANDROID_API="${ANDROID_API:-21}"
NDK_ROOT="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
BUILD_ABIS="arm64-v8a armeabi-v7a"
JOBS="${JOBS:-$(nproc 2>/dev/null || echo 4)}"
OPUS_VERSION="${OPUS_VERSION:-1.5.2}"
OPUS_TARBALL_URL="${OPUS_TARBALL_URL:-https://downloads.xiph.org/releases/opus/opus-${OPUS_VERSION}.tar.gz}"
OPUS_TARBALL_SHA256="${OPUS_TARBALL_SHA256:-65c1d2f78b9f2fb20082c38cbe47c951ad5839345876e46941612ee87f9a7ce1}"
LOCAL_OPUS_TARBALL="${LOCAL_OPUS_TARBALL:-${PROJECT_ROOT}/opus-${OPUS_VERSION}.tar.gz}"

usage() {
    cat <<'EOF'
Usage:
  scripts/build-android-opus.sh [options]

Options:
  --source <dir>          Existing opus source checkout. If omitted, the script
                          downloads and extracts the official source tarball
                          into .tmp/android-opus-src.
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
    --source)
        SOURCE_DIR="${2:-}"
        shift 2
        ;;
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

if [[ -z "$SOURCE_DIR" ]]; then
    SOURCE_DIR="${PROJECT_ROOT}/.tmp/android-opus-src"
    if [[ ! -f "$SOURCE_DIR/configure" ]]; then
        rm -rf "$SOURCE_DIR"
        mkdir -p "$SOURCE_DIR"
        tarball="${PROJECT_ROOT}/.tmp/opus-${OPUS_VERSION}.tar.gz"
        if [[ -f "$LOCAL_OPUS_TARBALL" ]]; then
            cp "$LOCAL_OPUS_TARBALL" "$tarball"
        else
            command -v curl >/dev/null 2>&1 || fail "curl is required to download opus source"
            curl -fsSL "$OPUS_TARBALL_URL" -o "$tarball"
        fi
        echo "${OPUS_TARBALL_SHA256}  ${tarball}" | sha256sum -c -
        tar -xzf "$tarball" -C "$SOURCE_DIR" --strip-components=1
    fi
fi

[[ -d "$SOURCE_DIR" ]] || fail "opus source not found: $SOURCE_DIR"
[[ -x "$SOURCE_DIR/configure" || -f "$SOURCE_DIR/configure.ac" ]] || fail "opus source layout not recognized under: $SOURCE_DIR"

SOURCE_DIR="$(cd "$SOURCE_DIR" && pwd)"
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
TOOLCHAIN="${NDK_ROOT}/toolchains/llvm/prebuilt/${HOST_TAG}"
ANDROID_TOOLCHAIN_FILE="${NDK_ROOT}/build/cmake/android.toolchain.cmake"
[[ -d "$TOOLCHAIN/bin" ]] || fail "NDK LLVM toolchain not found: $TOOLCHAIN"
[[ -f "$ANDROID_TOOLCHAIN_FILE" ]] || fail "NDK CMake toolchain not found: $ANDROID_TOOLCHAIN_FILE"
command -v cmake >/dev/null 2>&1 || fail "cmake is required"

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

    mkdir -p "$prefix/lib" "$prefix/include"
    if [[ -f "$prefix/include/opus/opus.h" ]]; then
        :
    elif [[ -f "$SOURCE_DIR/include/opus/opus.h" ]]; then
        mkdir -p "$prefix/include/opus"
        cp "$SOURCE_DIR/include/opus/opus.h" "$prefix/include/opus/opus.h"
    fi

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
