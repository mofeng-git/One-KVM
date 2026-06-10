#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

OUTPUT_DIR="${PROJECT_ROOT}/dist/android-alsa"
ANDROID_API="${ANDROID_API:-21}"
NDK_ROOT="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
BUILD_ABIS="arm64-v8a armeabi-v7a"
JOBS="${JOBS:-$(nproc 2>/dev/null || echo 4)}"
ALSA_VERSION="${ALSA_VERSION:-1.2.15}"

usage() {
    cat <<'EOF'
Usage:
  scripts/build-android-alsa.sh [options]

Options:
  --output <dir>          Output root. Default: dist/android-alsa
  --ndk <dir>             Android NDK root. Defaults to ANDROID_NDK_HOME or ANDROID_NDK_ROOT.
  --api <level>           Android API level. Default: 21.
  --abis <list>           Space/comma separated ABI list. Default: arm64-v8a armeabi-v7a.
  -h, --help              Show this help.

The output layout is compatible with ONE_KVM_ANDROID_ALSA_ROOT:
  <output>/arm64-v8a/include/alsa/asoundlib.h
  <output>/arm64-v8a/lib/libasound.so
  <output>/arm64-v8a/lib/pkgconfig/alsa.pc
  <output>/armeabi-v7a/include/alsa/asoundlib.h
  <output>/armeabi-v7a/lib/libasound.so
  <output>/armeabi-v7a/lib/pkgconfig/alsa.pc
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

SOURCE_DIR="${PROJECT_ROOT}/.tmp/android-alsa-src"
rm -rf "$SOURCE_DIR"
mkdir -p "${PROJECT_ROOT}/.tmp"
archive="${PROJECT_ROOT}/.tmp/alsa-lib-${ALSA_VERSION}.tar.bz2"
url="https://www.alsa-project.org/files/pub/lib/alsa-lib-${ALSA_VERSION}.tar.bz2"
echo "Downloading ALSA ${ALSA_VERSION}: $url"
curl -fL "$url" -o "$archive"
tar -xjf "$archive" -C "${PROJECT_ROOT}/.tmp"
mv "${PROJECT_ROOT}/.tmp/alsa-lib-${ALSA_VERSION}" "$SOURCE_DIR"

SOURCE_DIR="$(cd "$SOURCE_DIR" && pwd)"
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
TOOLCHAIN="${NDK_ROOT}/toolchains/llvm/prebuilt/${HOST_TAG}"

normalize_abis() {
    printf '%s\n' "$BUILD_ABIS" | tr ',' ' '
}

clean_generated_source_headers() {
    rm -f \
        "$SOURCE_DIR/include/asoundlib.h" \
        "$SOURCE_DIR/include/version.h" \
        "$SOURCE_DIR/include/stamp-vh" \
        "$SOURCE_DIR/include/alsa"
}

build_one() {
    local abi="$1"
    local prefix build_dir

    case "$abi" in
    arm64-v8a | armeabi-v7a) ;;
    *) fail "Unsupported ABI: $abi" ;;
    esac

    prefix="${OUTPUT_DIR}/${abi}"
    build_dir="${PROJECT_ROOT}/.tmp/alsa-android-build/${abi}"

    rm -rf "$build_dir"
    mkdir -p "$build_dir" "$prefix"

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

    clean_generated_source_headers

    if [[ ! -x "$SOURCE_DIR/configure" ]]; then
        (
            cd "$SOURCE_DIR"
            autoreconf -fi
        )
    fi

    (
        cd "$build_dir"
        pcm_plugins="copy linear route mulaw alaw adpcm rate plug multi file null empty meter hooks lfloat ladspa asym iec958 softvol extplug ioplug mmap_emul"
        ctl_plugins="remap ext"
        ac_cv_header_sys_shm_h=no \
        "$SOURCE_DIR/configure" \
            --host="$HOST_TRIPLE" \
            --prefix="$prefix" \
            --enable-shared \
            --disable-static \
            --disable-python \
            --with-pcm-plugins="$pcm_plugins" \
            --with-ctl-plugins="$ctl_plugins" \
            --disable-doc \
            --disable-oss \
            --disable-seq \
            --disable-ucm \
            --disable-topology \
            --disable-rawmidi \
            --disable-hwdep \
            --disable-usb \
            --disable-firewire \
            --disable-instr \
            --disable-alisp
        make -j"$JOBS"
        make install
    )

    mkdir -p "$prefix/lib/pkgconfig"
    cat > "$prefix/lib/pkgconfig/alsa.pc" <<EOF
prefix=\${pcfiledir}/../..
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: alsa
Description: ALSA sound library
Version: 1.2.15
Libs: -L\${libdir} -lasound
Cflags: -I\${includedir}
EOF

    echo "Built ALSA for ${abi}: ${prefix}"
}

for abi in $(normalize_abis); do
    build_one "$abi"
done

cat <<EOF

Done.

Use this when building the Android APK:
  export ONE_KVM_ANDROID_ALSA_ROOT="${OUTPUT_DIR}"
  cd android && ./gradlew :app:assembleDebug
EOF
