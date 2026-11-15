#!/bin/bash
set -euo pipefail
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build/cross/versions.env
source "$script_dir/versions.env"
architecture="${1:?architecture required}"
library="${2:?library required}"
prefix=/usr/local
triplet=''
processor=x86_64
cpu=x86_64
cmake_target=()
configure_target=()
case "$architecture" in
    armv7) triplet=arm-linux-gnueabihf; processor=arm; cpu=armv7; vpx_target=armv7-linux-gcc ;;
    arm64) triplet=aarch64-linux-gnu; processor=aarch64; cpu=aarch64; vpx_target=arm64-linux-gcc ;;
    x86_64) ;;
    *) echo "Unsupported architecture: $architecture" >&2; exit 1 ;;
esac
if [[ -n "$triplet" ]]; then
    prefix="/usr/$triplet"
    export CC="$triplet-gcc" CXX="$triplet-g++" LD="$triplet-ld"
    export AR="$triplet-ar" RANLIB="$triplet-ranlib" CROSS="$triplet-"
    cmake_target=(-DCMAKE_SYSTEM_NAME=Linux "-DCMAKE_SYSTEM_PROCESSOR=$processor"
        "-DCMAKE_C_COMPILER=$CC" "-DCMAKE_CXX_COMPILER=$CXX")
    configure_target=("--host=$triplet")
fi
github_prefix=''
if [[ "${CHINAMIRRO:-0}" == 1 ]]; then github_prefix="${GH_PROXY%/}/"; fi

checkout() {
    local repository="$1" revision="$2" directory="$3"
    git init "$directory"
    cd "$directory"
    git remote add origin "${github_prefix}https://github.com/$repository"
    git fetch --depth 1 origin "$revision"
    git checkout --detach FETCH_HEAD
}
install_make() {
    make -j"$(nproc)"
    make install
}
case "$library" in
    jpeg)
        checkout libjpeg-turbo/libjpeg-turbo.git "refs/tags/$LIBJPEG_TURBO_VERSION" /tmp/libjpeg-turbo
        cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
            "-DCMAKE_INSTALL_PREFIX=$prefix" -DCMAKE_INSTALL_LIBDIR=lib "${cmake_target[@]}" \
            -DENABLE_SHARED=OFF -DENABLE_STATIC=ON
        cd build
        install_make
        rm -rf /tmp/libjpeg-turbo
        ;;
    yuv)
        checkout lemenkov/libyuv "$LIBYUV_REV" /tmp/libyuv
        cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
            "-DCMAKE_INSTALL_PREFIX=$prefix" "${cmake_target[@]}" -DJPEG_FOUND=TRUE \
            "-DJPEG_INCLUDE_DIR=$prefix/include" "-DJPEG_LIBRARY=$prefix/lib/libjpeg.a" \
            "-DCMAKE_C_FLAGS=-DHAVE_JPEG -I$prefix/include" \
            "-DCMAKE_CXX_FLAGS=-DHAVE_JPEG -I$prefix/include"
        cd build
        install_make
        bash "$script_dir/libyuv-pkgconfig.sh" "$prefix" "$prefix/lib" "$prefix/lib/pkgconfig"
        rm -rf /tmp/libyuv
        ;;
    vpx)
        checkout webmproject/libvpx.git "refs/tags/v$LIBVPX_VERSION" /tmp/libvpx
        args=()
        if [[ -n "$triplet" ]]; then args=("--target=$vpx_target"); fi
        ./configure "--prefix=$prefix" "${args[@]}" --enable-static --disable-shared \
            --enable-pic --disable-examples --disable-tools --disable-docs --disable-unit-tests
        install_make
        rm -rf /tmp/libvpx
        ;;
    x264)
        git clone --depth 1 "${github_prefix}https://github.com/mirror/x264.git" /tmp/x264
        cd /tmp/x264
        args=()
        if [[ -n "$triplet" ]]; then args=("--cross-prefix=$triplet-"); fi
        ./configure "--prefix=$prefix" "${configure_target[@]}" "${args[@]}" \
            --enable-static --enable-pic --disable-cli
        install_make
        rm -rf /tmp/x264
        ;;
    x265)
        checkout videolan/x265.git "refs/tags/$X265_VERSION" /tmp/x265
        args=()
        if [[ "$architecture" == arm64 ]]; then
            args=(-DENABLE_NEON_DOTPROD=OFF -DENABLE_NEON_I8MM=OFF -DENABLE_SVE=OFF
                -DENABLE_SVE2=OFF -DENABLE_SVE2_BITPERM=OFF)
        fi
        cmake -S source -B source/build -DCMAKE_BUILD_TYPE=Release \
            "-DCMAKE_INSTALL_PREFIX=$prefix" "${cmake_target[@]}" "${args[@]}" \
            -DCMAKE_ASM_NASM_FLAGS=-w-macro-params-legacy \
            -DENABLE_SHARED=OFF -DENABLE_CLI=OFF -DBUILD_SHARED_LIBS=OFF -DCMAKE_POSITION_INDEPENDENT_CODE=ON
        cd source/build
        install_make
        mkdir -p "$prefix/lib/pkgconfig"
        cat > "$prefix/lib/pkgconfig/x265.pc" <<EOF
prefix=$prefix
exec_prefix=\${prefix}
libdir=\${prefix}/lib
includedir=\${prefix}/include
Name: x265
Description: H.265/HEVC video encoder
Version: 199
Libs: -L\${libdir} -lx265 -lstdc++ -lm -ldl -lpthread
Cflags: -I\${includedir}
EOF
        rm -rf /tmp/x265
        ;;
    opus)
        wget -O /tmp/opus.tar.gz "${github_prefix}https://github.com/xiph/opus/releases/download/v$OPUS_VERSION/opus-$OPUS_VERSION.tar.gz"
        tar -xzf /tmp/opus.tar.gz -C /tmp
        cd "/tmp/opus-$OPUS_VERSION"
        ./configure "--prefix=$prefix" "${configure_target[@]}" --enable-static --disable-shared --disable-doc
        install_make
        rm -rf "/tmp/opus-$OPUS_VERSION" /tmp/opus.tar.gz
        ;;
    ffmpeg)
        # shellcheck source=build/cross/build-ffmpeg.sh
        source "$script_dir/build-ffmpeg.sh"
        ;;
    *) echo "Unknown library: $library" >&2; exit 1 ;;
esac
