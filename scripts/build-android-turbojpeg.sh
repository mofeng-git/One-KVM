#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

OUTPUT_DIR="${PROJECT_ROOT}/dist/android-turbojpeg"
ANDROID_API="${ANDROID_API:-21}"
NDK_ROOT="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
BUILD_ABIS="arm64-v8a armeabi-v7a"
JOBS="${JOBS:-$(nproc 2>/dev/null || echo 4)}"
LIBJPEG_TURBO_VERSION="${LIBJPEG_TURBO_VERSION:-3.1.4.1}"

usage() {
    cat <<'EOF'
Usage:
  scripts/build-android-turbojpeg.sh [options]

Options:
  --output <dir>          Output root. Default: dist/android-turbojpeg
  --ndk <dir>             Android NDK root. Defaults to ANDROID_NDK_HOME or ANDROID_NDK_ROOT.
  --api <level>           Android API level. Default: 21.
  --abis <list>           Space/comma separated ABI list. Default: arm64-v8a armeabi-v7a.
  -h, --help              Show this help.

The output layout is compatible with ONE_KVM_ANDROID_TURBOJPEG_ROOT:
  <output>/arm64-v8a/include/turbojpeg.h
  <output>/arm64-v8a/lib/libturbojpeg.a
  <output>/arm64-v8a/include/jpeglib.h
  <output>/arm64-v8a/lib/libjpeg.a
  <output>/armeabi-v7a/include/turbojpeg.h
  <output>/armeabi-v7a/lib/libturbojpeg.a
  <output>/armeabi-v7a/include/jpeglib.h
  <output>/armeabi-v7a/lib/libjpeg.a
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

SOURCE_DIR="${PROJECT_ROOT}/.tmp/android-turbojpeg-src"
rm -rf "$SOURCE_DIR"
repo_url="https://github.com/libjpeg-turbo/libjpeg-turbo.git"
if [[ "${CHINAMIRRO:-0}" == "1" ]]; then
    repo_url="${GH_PROXY:-https://gh-proxy.com}"
    repo_url="${repo_url%/}/https://github.com/libjpeg-turbo/libjpeg-turbo.git"
fi
echo "Cloning libjpeg-turbo ${LIBJPEG_TURBO_VERSION}: $repo_url"
git init "$SOURCE_DIR"
(
    cd "$SOURCE_DIR"
    git remote add origin "$repo_url"
    git fetch --depth 1 origin "refs/tags/$LIBJPEG_TURBO_VERSION"
    git checkout --detach FETCH_HEAD
)

SOURCE_DIR="$(cd "$SOURCE_DIR" && pwd)"
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
ANDROID_TOOLCHAIN_FILE="${NDK_ROOT}/build/cmake/android.toolchain.cmake"
[[ -f "$ANDROID_TOOLCHAIN_FILE" ]] || fail "NDK CMake toolchain not found: $ANDROID_TOOLCHAIN_FILE"

normalize_abis() {
    printf '%s\n' "$BUILD_ABIS" | tr ',' ' '
}

build_one() {
    local abi="$1"
    local prefix build_dir lib_path

    case "$abi" in
    arm64-v8a | armeabi-v7a | x86 | x86_64) ;;
    *) fail "Unsupported ABI: $abi" ;;
    esac

    prefix="${OUTPUT_DIR}/${abi}"
    build_dir="${PROJECT_ROOT}/.tmp/turbojpeg-android-build/${abi}"

    rm -rf "$build_dir"
    mkdir -p "$build_dir" "$prefix"

    cmake -S "$SOURCE_DIR" -B "$build_dir" \
        -DCMAKE_TOOLCHAIN_FILE="$ANDROID_TOOLCHAIN_FILE" \
        -DANDROID_ABI="$abi" \
        -DANDROID_PLATFORM="android-${ANDROID_API}" \
        -DANDROID_STL=c++_shared \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$prefix" \
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
        -DCMAKE_C_FLAGS="-DANDROID -Dstderr=__sF+2" \
        -DCMAKE_CXX_FLAGS="-DANDROID -Dstderr=__sF+2" \
        -DENABLE_SHARED=OFF \
        -DENABLE_STATIC=ON \
        -DWITH_TURBOJPEG=ON \
        -DWITH_JAVA=OFF \
        -DWITH_12BIT=OFF \
        -DWITH_ARITH_DEC=ON \
        -DWITH_ARITH_ENC=ON

    cmake --build "$build_dir" --target turbojpeg-static jpeg-static --parallel "$JOBS"

    mkdir -p "$prefix/lib" "$prefix/include"
    lib_path="$build_dir/libturbojpeg.a"
    if [[ ! -f "$lib_path" ]]; then
        lib_path="$build_dir/lib/libturbojpeg.a"
    fi
    [[ -f "$lib_path" ]] || fail "Built libturbojpeg.a was not found under: $build_dir"

    cp "$lib_path" "$prefix/lib/libturbojpeg.a"
    lib_path="$build_dir/libjpeg.a"
    if [[ ! -f "$lib_path" ]]; then
        lib_path="$build_dir/lib/libjpeg.a"
    fi
    [[ -f "$lib_path" ]] || fail "Built libjpeg.a was not found under: $build_dir"

    cp "$lib_path" "$prefix/lib/libjpeg.a"
    cp "$SOURCE_DIR/src/turbojpeg.h" "$prefix/include/turbojpeg.h"
    cp "$SOURCE_DIR/src/jerror.h" "$prefix/include/jerror.h"
    cp "$SOURCE_DIR/src/jmorecfg.h" "$prefix/include/jmorecfg.h"
    cp "$SOURCE_DIR/src/jpeglib.h" "$prefix/include/jpeglib.h"
    cp "$build_dir/jconfig.h" "$prefix/include/jconfig.h"

    echo "Built TurboJPEG for ${abi}: ${prefix}"
}

for abi in $(normalize_abis); do
    build_one "$abi"
done

cat <<EOF

Done.

Use this when building the Android APK:
  export ONE_KVM_ANDROID_TURBOJPEG_ROOT="${OUTPUT_DIR}"
  cd android && ./gradlew :app:assembleDebug
EOF
