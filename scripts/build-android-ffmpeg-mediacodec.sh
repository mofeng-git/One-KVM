#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

OUTPUT_DIR="${PROJECT_ROOT}/dist/android-ffmpeg-mediacodec"
ANDROID_API="${ANDROID_API:-21}"
NDK_ROOT="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
BUILD_ABIS="arm64-v8a armeabi-v7a"
JOBS="${JOBS:-$(nproc 2>/dev/null || echo 4)}"
FFMPEG_ROCKCHIP_REV="${FFMPEG_ROCKCHIP_REV:-40c412daccf08164493da0de990eb99a8948116b}"

usage() {
    cat <<'EOF'
Usage:
  scripts/build-android-ffmpeg-mediacodec.sh [options]

Options:
  --output <dir>          Output root. Default: dist/android-ffmpeg-mediacodec
  --ndk <dir>             Android NDK root. Defaults to ANDROID_NDK_HOME or ANDROID_NDK_ROOT.
  --api <level>           Android API level. Default: 21.
  --abis <list>           Space/comma separated ABI list. Default: arm64-v8a armeabi-v7a.
  -h, --help              Show this help.

The output layout is compatible with ONE_KVM_ANDROID_FFMPEG_ROOT:
  <output>/arm64-v8a/include
  <output>/arm64-v8a/lib
  <output>/armeabi-v7a/include
  <output>/armeabi-v7a/lib

Example:
  scripts/build-android-ffmpeg-mediacodec.sh --output /opt/one-kvm/android-ffmpeg

  export ONE_KVM_ANDROID_FFMPEG_ROOT=/opt/one-kvm/android-ffmpeg
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
    -h | --help)
        usage
        exit 0
        ;;
    *)
        fail "Unknown argument: $1"
        ;;
    esac
done

SOURCE_DIR="${PROJECT_ROOT}/.tmp/android-ffmpeg-check/src/ffmpeg-rockchip"
rm -rf "$SOURCE_DIR"
mkdir -p "$(dirname "$SOURCE_DIR")"
repo_url="https://github.com/nyanmisaka/ffmpeg-rockchip.git"
if [[ "${CHINAMIRRO:-0}" == "1" ]]; then
    repo_url="${GH_PROXY:-https://gh-proxy.com}"
    repo_url="${repo_url%/}/https://github.com/nyanmisaka/ffmpeg-rockchip.git"
fi
echo "Cloning FFmpeg source: $repo_url"
git init "$SOURCE_DIR"
(
    cd "$SOURCE_DIR"
    git remote add origin "$repo_url"
    git fetch --depth 1 origin "$FFMPEG_ROCKCHIP_REV"
    git checkout --detach FETCH_HEAD
)

[[ -n "$NDK_ROOT" ]] || fail "--ndk or ANDROID_NDK_HOME/ANDROID_NDK_ROOT is required"

SOURCE_DIR="$(cd "$SOURCE_DIR" && pwd)"
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
TOOLCHAIN="${NDK_ROOT}/toolchains/llvm/prebuilt/${HOST_TAG}"

normalize_abis() {
    printf '%s\n' "$BUILD_ABIS" | tr ',' ' '
}

patch_android_ffmpeg_mjpeg_mediacodec() {
    local avcodec_dir="${SOURCE_DIR}/libavcodec"
    local configure_file="${SOURCE_DIR}/configure"
    local mediacodecdec="${avcodec_dir}/mediacodecdec.c"
    local allcodecs="${avcodec_dir}/allcodecs.c"
    local makefile="${avcodec_dir}/Makefile"

    python3 - "$mediacodecdec" "$allcodecs" "$configure_file" "$makefile" <<'PY'
from pathlib import Path
import sys

mediacodecdec, allcodecs, configure_file, makefile = map(Path, sys.argv[1:])

def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    if new in text:
        return
    if old not in text:
        raise SystemExit(f"patch anchor not found in {path}: {old!r}")
    path.write_text(text.replace(old, new, 1))

replace_once(
    mediacodecdec,
    "CONFIG_MPEG2_MEDIACODEC_DECODER || \\\n",
    "CONFIG_MJPEG_MEDIACODEC_DECODER || \\\n"
    "    CONFIG_MPEG2_MEDIACODEC_DECODER || \\\n",
)
replace_once(
    mediacodecdec,
    "#if CONFIG_MPEG2_MEDIACODEC_DECODER\n"
    "    case AV_CODEC_ID_MPEG2VIDEO:",
    "#if CONFIG_MJPEG_MEDIACODEC_DECODER\n"
    "    case AV_CODEC_ID_MJPEG:\n"
    "        codec_mime = \"video/mjpeg\";\n\n"
    "        ret = common_set_extradata(avctx, format);\n"
    "        if (ret < 0)\n"
    "            goto done;\n"
    "        break;\n"
    "#endif\n"
    "#if CONFIG_MPEG2_MEDIACODEC_DECODER\n"
    "    case AV_CODEC_ID_MPEG2VIDEO:",
)
replace_once(
    mediacodecdec,
    "#if CONFIG_MPEG2_MEDIACODEC_DECODER\n"
    "DECLARE_MEDIACODEC_VDEC(mpeg2, \"MPEG-2\", AV_CODEC_ID_MPEG2VIDEO, NULL)",
    "#if CONFIG_MJPEG_MEDIACODEC_DECODER\n"
    "DECLARE_MEDIACODEC_VDEC(mjpeg, \"MJPEG\", AV_CODEC_ID_MJPEG, NULL)\n"
    "#endif\n\n"
    "#if CONFIG_MPEG2_MEDIACODEC_DECODER\n"
    "DECLARE_MEDIACODEC_VDEC(mpeg2, \"MPEG-2\", AV_CODEC_ID_MPEG2VIDEO, NULL)",
)
replace_once(
    allcodecs,
    "extern const FFCodec ff_mjpeg_cuvid_decoder;",
    "extern const FFCodec ff_mjpeg_cuvid_decoder;\n"
    "extern const FFCodec ff_mjpeg_mediacodec_decoder;",
)
replace_once(
    configure_file,
    'mjpeg_cuvid_decoder_deps="cuvid"',
    'mjpeg_cuvid_decoder_deps="cuvid"\n'
    'mjpeg_mediacodec_decoder_deps="mediacodec"',
)
replace_once(
    makefile,
    "OBJS-$(CONFIG_MJPEG_RKMPP_DECODER)",
    "OBJS-$(CONFIG_MJPEG_MEDIACODEC_DECODER) += mediacodecdec.o\n"
    "OBJS-$(CONFIG_MJPEG_RKMPP_DECODER)",
)
PY
}

abi_arch() {
    case "$1" in
    arm64-v8a) echo "aarch64" ;;
    armeabi-v7a) echo "arm" ;;
    *) fail "Unsupported ABI: $1" ;;
    esac
}

abi_cpu() {
    case "$1" in
    arm64-v8a) echo "armv8-a" ;;
    armeabi-v7a) echo "armv7-a" ;;
    *) fail "Unsupported ABI: $1" ;;
    esac
}

abi_target() {
    case "$1" in
    arm64-v8a) echo "aarch64-linux-android" ;;
    armeabi-v7a) echo "armv7a-linux-androideabi" ;;
    *) fail "Unsupported ABI: $1" ;;
    esac
}

build_one() {
    local abi="$1"
    local arch cpu target prefix build_dir cc cxx ar ranlib strip extra_cflags extra_ldflags

    arch="$(abi_arch "$abi")"
    cpu="$(abi_cpu "$abi")"
    target="$(abi_target "$abi")"
    prefix="${OUTPUT_DIR}/${abi}"
    build_dir="${PROJECT_ROOT}/.tmp/ffmpeg-android-build/${abi}"
    cc="${TOOLCHAIN}/bin/${target}${ANDROID_API}-clang"
    cxx="${TOOLCHAIN}/bin/${target}${ANDROID_API}-clang++"
    ar="${TOOLCHAIN}/bin/llvm-ar"
    ranlib="${TOOLCHAIN}/bin/llvm-ranlib"
    strip="${TOOLCHAIN}/bin/llvm-strip"
    extra_cflags="-fPIC"
    extra_ldflags=""

    if [[ "$abi" == "armeabi-v7a" ]]; then
        extra_cflags="${extra_cflags} -march=armv7-a -mfloat-abi=softfp -mfpu=neon"
        extra_ldflags="${extra_ldflags} -Wl,--fix-cortex-a8"
    fi

    rm -rf "$build_dir"
    mkdir -p "$build_dir" "$prefix"

    (
        cd "$build_dir"
        "${SOURCE_DIR}/configure" \
            --prefix="$prefix" \
            --target-os=android \
            --arch="$arch" \
            --cpu="$cpu" \
            --cc="$cc" \
            --cxx="$cxx" \
            --ar="$ar" \
            --ranlib="$ranlib" \
            --strip="$strip" \
            --cross-prefix="${TOOLCHAIN}/bin/llvm-" \
            --sysroot="${TOOLCHAIN}/sysroot" \
            --enable-cross-compile \
            --enable-static \
            --disable-shared \
            --disable-programs \
            --disable-doc \
            --disable-avdevice \
            --disable-avformat \
            --disable-avfilter \
            --disable-swscale \
            --disable-swresample \
            --disable-postproc \
            --disable-network \
            --disable-everything \
            --disable-hwaccels \
            --disable-cuda-llvm \
            --disable-v4l2-m2m \
            --disable-vulkan \
            --enable-pthreads \
            --enable-jni \
            --enable-mediacodec \
            --enable-decoder=mjpeg_mediacodec \
            --enable-decoder=mjpeg \
            --enable-encoder=h264_mediacodec \
            --enable-encoder=hevc_mediacodec \
            --enable-parser=mjpeg \
            --enable-bsf=h264_metadata \
            --enable-bsf=hevc_metadata \
            --enable-protocol=file \
            --extra-cflags="$extra_cflags" \
            --extra-ldflags="$extra_ldflags"

        make -j"$JOBS"
        make install
    )

    echo "Built FFmpeg MediaCodec for ${abi}: ${prefix}"
}

patch_android_ffmpeg_mjpeg_mediacodec

for abi in $(normalize_abis); do
    build_one "$abi"
done

cat <<EOF

Done.

Use this when building the Android APK:
  export ONE_KVM_ANDROID_FFMPEG_ROOT="${OUTPUT_DIR}"
  cd android && ./gradlew :app:assembleDebug
EOF
