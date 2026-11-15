#!/bin/bash
# Sourced by build-native.sh; target variables and checkout/install_make are shared.
set -euo pipefail
# shellcheck disable=SC2154
if [[ -n "$triplet" ]]; then
    mkdir -p /tmp/ffmpeg-build
    cd /tmp/ffmpeg-build
    git clone --depth 1 --branch "$RKMPP_BRANCH" "${github_prefix}https://github.com/nyanmisaka/mpp.git" rkmpp
    git clone --depth 1 --branch "$RKRGA_BRANCH" "${github_prefix}https://github.com/nyanmisaka/rk-mirrors.git" rkrga
    (
        cd rkmpp
        cmake -S . -B build "${cmake_target[@]}" "-DCMAKE_INSTALL_PREFIX=$prefix" \
            -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DBUILD_TEST=OFF
        cd build
        install_make
        sed -i 's/^Libs:.*$/& -lstdc++ -lm -lpthread/' "$prefix/lib/pkgconfig/rockchip_mpp.pc"
        rm -f "$prefix"/lib/librockchip_mpp.so*
    )
    cat > /tmp/one-kvm-cross.txt <<EOF
[binaries]
c = '$CC'
cpp = '$CXX'
ar = '$AR'
strip = '$triplet-strip'
pkgconfig = 'pkg-config'
[host_machine]
system = 'linux'
cpu_family = '$processor'
cpu = '$cpu'
endian = 'little'
EOF
    (
        cd rkrga
        meson setup build "--prefix=$prefix" --libdir=lib --cross-file=/tmp/one-kvm-cross.txt \
            --buildtype=release --default-library=static -Dcpp_args=-fpermissive \
            -Dlibdrm=false -Dlibrga_demo=false
        ninja -C build
        ninja -C build install
        # Upstream uses shared_library; retain the existing archive assembly.
        mapfile -d '' objects < <(find build -name '*.o' -print0)
        "$AR" rcs "$prefix/lib/librga.a" "${objects[@]}"
        "$RANLIB" "$prefix/lib/librga.a"
        sed -i 's/^Libs:.*$/& -lstdc++ -lm -lpthread/' "$prefix/lib/pkgconfig/librga.pc"
        rm -f "$prefix"/lib/librga.so*
    )
    cat > /tmp/one-kvm-pkg-config <<EOF
#!/bin/sh
export PKG_CONFIG_LIBDIR=$prefix/lib/pkgconfig:/usr/lib/$triplet/pkgconfig
export PKG_CONFIG_PATH=""
export PKG_CONFIG_SYSROOT_DIR=""
exec pkg-config "\$@"
EOF
    chmod +x /tmp/one-kvm-pkg-config
    hardware=("--prefix=$prefix" "--cross-prefix=$triplet-" "--arch=$processor"
        --target-os=linux --enable-cross-compile --pkg-config=/tmp/one-kvm-pkg-config
        --enable-rkmpp --enable-rkrga --disable-vaapi --enable-v4l2-m2m
        --enable-decoder=mjpeg --enable-decoder=mjpeg_rkmpp
        --enable-encoder=h264_rkmpp --enable-encoder=hevc_rkmpp
        --enable-encoder=h264_v4l2m2m --enable-encoder=hevc_v4l2m2m)
else
    hardware=(--enable-vaapi --enable-libmfx --disable-hwaccels
        --enable-encoder=h264_vaapi --enable-encoder=hevc_vaapi
        --enable-encoder=vp8_vaapi --enable-encoder=vp9_vaapi
        --enable-encoder=h264_qsv --enable-encoder=hevc_qsv)
fi
checkout nyanmisaka/ffmpeg-rockchip.git "$FFMPEG_ROCKCHIP_REV" /tmp/ffmpeg-build/ffmpeg-rockchip
if [[ -n "$triplet" ]]; then patch -p1 < /tmp/ffmpeg-v4l2-size.patch; fi
./configure --enable-gpl --enable-version3 --disable-shared --enable-static --enable-pic \
    --enable-libdrm --enable-libx264 --enable-libx265 --enable-libvpx \
    --disable-programs --disable-doc --disable-htmlpages --disable-manpages \
    --disable-podpages --disable-txtpages --disable-network --disable-protocols \
    --disable-avformat --disable-swscale --disable-swresample --disable-avfilter \
    --disable-avdevice --disable-postproc --disable-decoders --disable-encoders \
    --enable-encoder=libx264 --enable-encoder=libx265 \
    --enable-encoder=libvpx_vp8 --enable-encoder=libvpx_vp9 \
    --disable-muxers --disable-demuxers --disable-parsers \
    --enable-parser=h264 --enable-parser=hevc --enable-parser=vp8 --enable-parser=vp9 \
    --disable-bsfs --enable-bsf=h264_mp4toannexb --enable-bsf=hevc_mp4toannexb \
    --disable-indevs --disable-outdevs --disable-filters --disable-debug "${hardware[@]}"
install_make
cd /
rm -rf /tmp/ffmpeg-build /tmp/one-kvm-cross.txt /tmp/one-kvm-pkg-config
