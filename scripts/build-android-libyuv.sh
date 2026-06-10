#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

OUTPUT_DIR="${PROJECT_ROOT}/dist/android-libyuv"
JPEG_ROOT="${ONE_KVM_ANDROID_TURBOJPEG_ROOT:-${PROJECT_ROOT}/dist/android-turbojpeg}"
ANDROID_API="${ANDROID_API:-21}"
NDK_ROOT="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
BUILD_ABIS="arm64-v8a armeabi-v7a"
JOBS="${JOBS:-$(nproc 2>/dev/null || echo 4)}"
LIBYUV_REV="${LIBYUV_REV:-957f295ea946cbbd13fcfc46e7066f2efa801233}"

usage() {
    cat <<'EOF'
Usage:
  scripts/build-android-libyuv.sh [options]

Options:
  --output <dir>          Output root. Default: dist/android-libyuv
  --ndk <dir>             Android NDK root. Defaults to ANDROID_NDK_HOME or ANDROID_NDK_ROOT.
  --api <level>           Android API level. Default: 21.
  --abis <list>           Space/comma separated ABI list. Default: arm64-v8a armeabi-v7a.
  --jpeg-root <dir>       Android libjpeg root. Defaults to ONE_KVM_ANDROID_TURBOJPEG_ROOT
                          or dist/android-turbojpeg when present. Enables libyuv HAVE_JPEG.
  -h, --help              Show this help.

The output layout is compatible with ONE_KVM_ANDROID_LIBYUV_ROOT:
  <output>/arm64-v8a/include
  <output>/arm64-v8a/lib/libyuv.a
  <output>/armeabi-v7a/include
  <output>/armeabi-v7a/lib/libyuv.a

Example:
  scripts/build-android-libyuv.sh --output /opt/one-kvm/android-libyuv

  export ONE_KVM_ANDROID_LIBYUV_ROOT=/opt/one-kvm/android-libyuv
  cd android && ./gradlew :app:assembleDebug
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
    --jpeg-root)
        JPEG_ROOT="${2:-}"
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

SOURCE_DIR="${PROJECT_ROOT}/.tmp/android-libyuv-src"
rm -rf "$SOURCE_DIR"
repo_url="https://github.com/lemenkov/libyuv.git"
if [[ "${CHINAMIRRO:-0}" == "1" ]]; then
    repo_url="${GH_PROXY:-https://gh-proxy.com}"
    repo_url="${repo_url%/}/https://github.com/lemenkov/libyuv.git"
fi
echo "Cloning libyuv source: $repo_url"
git init "$SOURCE_DIR"
(
    cd "$SOURCE_DIR"
    git remote add origin "$repo_url"
    git fetch --depth 1 origin "$LIBYUV_REV"
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
    local prefix build_dir jpeg_include jpeg_library
    local -a jpeg_args

    case "$abi" in
    arm64-v8a | armeabi-v7a | x86 | x86_64) ;;
    *) fail "Unsupported ABI: $abi" ;;
    esac

    prefix="${OUTPUT_DIR}/${abi}"
    build_dir="${PROJECT_ROOT}/.tmp/libyuv-android-build/${abi}"

    rm -rf "$build_dir"
    mkdir -p "$build_dir" "$prefix"

    jpeg_include="$JPEG_ROOT/$abi/include"
    jpeg_library="$JPEG_ROOT/$abi/lib/libjpeg.a"
    jpeg_args=()
    if [[ -f "$jpeg_library" && -f "$jpeg_include/jpeglib.h" ]]; then
        jpeg_args=(
            -DJPEG_FOUND=TRUE
            -DJPEG_INCLUDE_DIR="$jpeg_include"
            -DJPEG_LIBRARY="$jpeg_library"
            -DCMAKE_C_FLAGS="-DHAVE_JPEG"
            -DCMAKE_CXX_FLAGS="-DHAVE_JPEG"
        )
    else
        echo "Warning: Android libjpeg not found for ${abi}; libyuv MJPEG APIs will be disabled." >&2
        echo "         Checked: $jpeg_library and $jpeg_include/jpeglib.h" >&2
    fi

    cmake -S "$SOURCE_DIR" -B "$build_dir" \
        -DCMAKE_TOOLCHAIN_FILE="$ANDROID_TOOLCHAIN_FILE" \
        -DANDROID_ABI="$abi" \
        -DANDROID_PLATFORM="android-${ANDROID_API}" \
        -DANDROID_STL=c++_shared \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$prefix" \
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
        -DBUILD_SHARED_LIBS=OFF \
        -DUNIT_TEST=OFF \
        -DTEST=OFF \
        "${jpeg_args[@]}"

    cmake --build "$build_dir" --target yuv --parallel "$JOBS"

    mkdir -p "$prefix/lib" "$prefix/include"
    if [[ -f "$build_dir/libyuv.a" ]]; then
        cp "$build_dir/libyuv.a" "$prefix/lib/libyuv.a"
    elif [[ -f "$build_dir/lib/libyuv.a" ]]; then
        cp "$build_dir/lib/libyuv.a" "$prefix/lib/libyuv.a"
    else
        fail "Built libyuv.a was not found under: $build_dir"
    fi
    cp -R "$SOURCE_DIR/include/." "$prefix/include/"

    echo "Built libyuv for ${abi}: ${prefix}"
}

for abi in $(normalize_abis); do
    build_one "$abi"
done

cat <<EOF

Done.

Use this when building the Android APK:
  export ONE_KVM_ANDROID_LIBYUV_ROOT="${OUTPUT_DIR}"
  export ONE_KVM_ANDROID_TURBOJPEG_ROOT="${JPEG_ROOT}"
  cd android && ./gradlew :app:assembleDebug
EOF
