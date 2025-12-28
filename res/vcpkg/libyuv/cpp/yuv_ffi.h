#ifndef YUV_FFI_H
#define YUV_FFI_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// libyuv function declarations
// These are the most commonly used functions for video format conversion
// ============================================================================

// ----------------------------------------------------------------------------
// YUYV (YUY2) conversions - Common for USB capture cards
// ----------------------------------------------------------------------------

// YUYV -> I420 (YUV420P planar)
int YUY2ToI420(const uint8_t* src_yuy2, int src_stride_yuy2,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_u, int dst_stride_u,
               uint8_t* dst_v, int dst_stride_v,
               int width, int height);

// YUYV -> NV12 (optimal for VAAPI)
int YUY2ToNV12(const uint8_t* src_yuy2, int src_stride_yuy2,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_uv, int dst_stride_uv,
               int width, int height);

// ----------------------------------------------------------------------------
// UYVY conversions
// ----------------------------------------------------------------------------

int UYVYToI420(const uint8_t* src_uyvy, int src_stride_uyvy,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_u, int dst_stride_u,
               uint8_t* dst_v, int dst_stride_v,
               int width, int height);

int UYVYToNV12(const uint8_t* src_uyvy, int src_stride_uyvy,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_uv, int dst_stride_uv,
               int width, int height);

// ----------------------------------------------------------------------------
// I420 (YUV420P) conversions
// ----------------------------------------------------------------------------

// I422 (YUV422P) -> I420 (YUV420P) with vertical chroma downsampling
int I422ToI420(const uint8_t* src_y, int src_stride_y,
               const uint8_t* src_u, int src_stride_u,
               const uint8_t* src_v, int src_stride_v,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_u, int dst_stride_u,
               uint8_t* dst_v, int dst_stride_v,
               int width, int height);

// I420 -> NV12
int I420ToNV12(const uint8_t* src_y, int src_stride_y,
               const uint8_t* src_u, int src_stride_u,
               const uint8_t* src_v, int src_stride_v,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_uv, int dst_stride_uv,
               int width, int height);

// I420 -> NV21
int I420ToNV21(const uint8_t* src_y, int src_stride_y,
               const uint8_t* src_u, int src_stride_u,
               const uint8_t* src_v, int src_stride_v,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_vu, int dst_stride_vu,
               int width, int height);

// ----------------------------------------------------------------------------
// NV12/NV21 conversions
// ----------------------------------------------------------------------------

// NV12 -> I420
int NV12ToI420(const uint8_t* src_y, int src_stride_y,
               const uint8_t* src_uv, int src_stride_uv,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_u, int dst_stride_u,
               uint8_t* dst_v, int dst_stride_v,
               int width, int height);

// NV21 -> I420
int NV21ToI420(const uint8_t* src_y, int src_stride_y,
               const uint8_t* src_vu, int src_stride_vu,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_u, int dst_stride_u,
               uint8_t* dst_v, int dst_stride_v,
               int width, int height);

// ----------------------------------------------------------------------------
// ARGB/BGRA conversions (32-bit RGB)
// Note: libyuv uses ARGB to mean BGRA in memory (little-endian)
// ----------------------------------------------------------------------------

// BGRA -> I420
int ARGBToI420(const uint8_t* src_argb, int src_stride_argb,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_u, int dst_stride_u,
               uint8_t* dst_v, int dst_stride_v,
               int width, int height);

// BGRA -> NV12
int ARGBToNV12(const uint8_t* src_argb, int src_stride_argb,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_uv, int dst_stride_uv,
               int width, int height);

// RGBA -> I420
int ABGRToI420(const uint8_t* src_abgr, int src_stride_abgr,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_u, int dst_stride_u,
               uint8_t* dst_v, int dst_stride_v,
               int width, int height);

// RGBA -> NV12
int ABGRToNV12(const uint8_t* src_abgr, int src_stride_abgr,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_uv, int dst_stride_uv,
               int width, int height);

// ARGB <-> ABGR swap (BGRA <-> RGBA)
int ARGBToABGR(const uint8_t* src_argb, int src_stride_argb,
               uint8_t* dst_abgr, int dst_stride_abgr,
               int width, int height);

int ABGRToARGB(const uint8_t* src_abgr, int src_stride_abgr,
               uint8_t* dst_argb, int dst_stride_argb,
               int width, int height);

// ----------------------------------------------------------------------------
// RGB24/BGR24 conversions (24-bit RGB)
// ----------------------------------------------------------------------------

// RGB24 -> I420
int RGB24ToI420(const uint8_t* src_rgb24, int src_stride_rgb24,
                uint8_t* dst_y, int dst_stride_y,
                uint8_t* dst_u, int dst_stride_u,
                uint8_t* dst_v, int dst_stride_v,
                int width, int height);

// BGR24 (RAW) -> I420
int RAWToI420(const uint8_t* src_raw, int src_stride_raw,
              uint8_t* dst_y, int dst_stride_y,
              uint8_t* dst_u, int dst_stride_u,
              uint8_t* dst_v, int dst_stride_v,
              int width, int height);

// RGB24 -> ARGB
int RGB24ToARGB(const uint8_t* src_rgb24, int src_stride_rgb24,
                uint8_t* dst_argb, int dst_stride_argb,
                int width, int height);

// BGR24 (RAW) -> ARGB
int RAWToARGB(const uint8_t* src_raw, int src_stride_raw,
              uint8_t* dst_argb, int dst_stride_argb,
              int width, int height);

// ----------------------------------------------------------------------------
// YUV to RGB conversions (for display/JPEG encoding)
// ----------------------------------------------------------------------------

// I420 -> RGB24
int I420ToRGB24(const uint8_t* src_y, int src_stride_y,
                const uint8_t* src_u, int src_stride_u,
                const uint8_t* src_v, int src_stride_v,
                uint8_t* dst_rgb24, int dst_stride_rgb24,
                int width, int height);

// I420 -> ARGB (BGRA)
int I420ToARGB(const uint8_t* src_y, int src_stride_y,
               const uint8_t* src_u, int src_stride_u,
               const uint8_t* src_v, int src_stride_v,
               uint8_t* dst_argb, int dst_stride_argb,
               int width, int height);

// NV12 -> RGB24
int NV12ToRGB24(const uint8_t* src_y, int src_stride_y,
                const uint8_t* src_uv, int src_stride_uv,
                uint8_t* dst_rgb24, int dst_stride_rgb24,
                int width, int height);

// NV12 -> ARGB (BGRA)
int NV12ToARGB(const uint8_t* src_y, int src_stride_y,
               const uint8_t* src_uv, int src_stride_uv,
               uint8_t* dst_argb, int dst_stride_argb,
               int width, int height);

// YUYV -> ARGB (BGRA)
int YUY2ToARGB(const uint8_t* src_yuy2, int src_stride_yuy2,
               uint8_t* dst_argb, int dst_stride_argb,
               int width, int height);

// UYVY -> ARGB (BGRA)
int UYVYToARGB(const uint8_t* src_uyvy, int src_stride_uyvy,
               uint8_t* dst_argb, int dst_stride_argb,
               int width, int height);

// ARGB -> RGB24
int ARGBToRGB24(const uint8_t* src_argb, int src_stride_argb,
                uint8_t* dst_rgb24, int dst_stride_rgb24,
                int width, int height);

// ARGB -> RAW (BGR24)
int ARGBToRAW(const uint8_t* src_argb, int src_stride_argb,
              uint8_t* dst_raw, int dst_stride_raw,
              int width, int height);

// ----------------------------------------------------------------------------
// MJPEG decoding (libyuv built-in, faster than FFmpeg for simple cases)
// ----------------------------------------------------------------------------

// MJPEG -> I420
int MJPGToI420(const uint8_t* sample, size_t sample_size,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_u, int dst_stride_u,
               uint8_t* dst_v, int dst_stride_v,
               int src_width, int src_height,
               int dst_width, int dst_height);

// MJPEG -> NV12
int MJPGToNV12(const uint8_t* sample, size_t sample_size,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_uv, int dst_stride_uv,
               int src_width, int src_height,
               int dst_width, int dst_height);

// MJPEG -> ARGB
int MJPGToARGB(const uint8_t* sample, size_t sample_size,
               uint8_t* dst_argb, int dst_stride_argb,
               int src_width, int src_height,
               int dst_width, int dst_height);

// Get MJPEG dimensions without decoding
int MJPGSize(const uint8_t* sample, size_t sample_size,
             int* width, int* height);

// ----------------------------------------------------------------------------
// Scaling
// ----------------------------------------------------------------------------

// Scale filter modes
enum FilterMode {
    kFilterNone = 0,      // Point sample; Fastest
    kFilterLinear = 1,    // Filter horizontally only
    kFilterBilinear = 2,  // Faster than box, but lower quality scaling down
    kFilterBox = 3        // Highest quality
};

// I420 scale
int I420Scale(const uint8_t* src_y, int src_stride_y,
              const uint8_t* src_u, int src_stride_u,
              const uint8_t* src_v, int src_stride_v,
              int src_width, int src_height,
              uint8_t* dst_y, int dst_stride_y,
              uint8_t* dst_u, int dst_stride_u,
              uint8_t* dst_v, int dst_stride_v,
              int dst_width, int dst_height,
              enum FilterMode filtering);

// NV12 scale
int NV12Scale(const uint8_t* src_y, int src_stride_y,
              const uint8_t* src_uv, int src_stride_uv,
              int src_width, int src_height,
              uint8_t* dst_y, int dst_stride_y,
              uint8_t* dst_uv, int dst_stride_uv,
              int dst_width, int dst_height,
              enum FilterMode filtering);

// ARGB scale
int ARGBScale(const uint8_t* src_argb, int src_stride_argb,
              int src_width, int src_height,
              uint8_t* dst_argb, int dst_stride_argb,
              int dst_width, int dst_height,
              enum FilterMode filtering);

// ----------------------------------------------------------------------------
// Rotation
// ----------------------------------------------------------------------------

enum RotationMode {
    kRotate0 = 0,
    kRotate90 = 90,
    kRotate180 = 180,
    kRotate270 = 270
};

// I420 rotate
int I420Rotate(const uint8_t* src_y, int src_stride_y,
               const uint8_t* src_u, int src_stride_u,
               const uint8_t* src_v, int src_stride_v,
               uint8_t* dst_y, int dst_stride_y,
               uint8_t* dst_u, int dst_stride_u,
               uint8_t* dst_v, int dst_stride_v,
               int width, int height,
               enum RotationMode mode);

// NV12 rotate
int NV12ToI420Rotate(const uint8_t* src_y, int src_stride_y,
                     const uint8_t* src_uv, int src_stride_uv,
                     uint8_t* dst_y, int dst_stride_y,
                     uint8_t* dst_u, int dst_stride_u,
                     uint8_t* dst_v, int dst_stride_v,
                     int width, int height,
                     enum RotationMode mode);

// ----------------------------------------------------------------------------
// Copy functions
// ----------------------------------------------------------------------------

// Copy I420
int I420Copy(const uint8_t* src_y, int src_stride_y,
             const uint8_t* src_u, int src_stride_u,
             const uint8_t* src_v, int src_stride_v,
             uint8_t* dst_y, int dst_stride_y,
             uint8_t* dst_u, int dst_stride_u,
             uint8_t* dst_v, int dst_stride_v,
             int width, int height);

// Copy NV12
int NV12Copy(const uint8_t* src_y, int src_stride_y,
             const uint8_t* src_uv, int src_stride_uv,
             uint8_t* dst_y, int dst_stride_y,
             uint8_t* dst_uv, int dst_stride_uv,
             int width, int height);

#ifdef __cplusplus
}
#endif

#endif // YUV_FFI_H
