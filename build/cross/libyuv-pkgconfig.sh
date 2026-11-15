#!/bin/bash
# Supply metadata for libyuv installations whose upstream build omits it.
# Usage: bash libyuv-pkgconfig.sh PREFIX LIBDIR OUTPUT_DIRECTORY
set -euo pipefail
prefix="${1:?installation prefix required}"
libdir="${2:?library directory required}"
output="${3:?pkg-config output directory required}"
[[ -f "$prefix/include/libyuv.h" ]] || { echo "Missing $prefix/include/libyuv.h" >&2; exit 1; }
[[ -f "$libdir/libyuv.a" || -f "$libdir/libyuv.so" ]] || { echo "Missing libyuv in $libdir" >&2; exit 1; }
mkdir -p "$output"
cat > "$output/libyuv.pc" <<EOF
prefix=$prefix
libdir=$libdir
includedir=\${prefix}/include
Name: libyuv
Description: YUV conversion library
Version: 0.0.0
Libs: -L\${libdir} -lyuv
Libs.private: -ljpeg -lstdc++ -lm
Cflags: -I\${includedir}
EOF
