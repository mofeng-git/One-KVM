#ifndef HWCODEC_FFMPEG_CAPTURE_FFI_H
#define HWCODEC_FFMPEG_CAPTURE_FFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct HwcodecDshowCaptureContext HwcodecDshowCaptureContext;

enum HwcodecCapturePixelFormat {
  HWCODEC_CAPTURE_FMT_UNKNOWN = 0,
  HWCODEC_CAPTURE_FMT_MJPEG = 1,
  HWCODEC_CAPTURE_FMT_JPEG = 2,
  HWCODEC_CAPTURE_FMT_YUYV = 3,
  HWCODEC_CAPTURE_FMT_YVYU = 4,
  HWCODEC_CAPTURE_FMT_UYVY = 5,
  HWCODEC_CAPTURE_FMT_NV12 = 6,
  HWCODEC_CAPTURE_FMT_NV21 = 7,
  HWCODEC_CAPTURE_FMT_NV16 = 8,
  HWCODEC_CAPTURE_FMT_NV24 = 9,
  HWCODEC_CAPTURE_FMT_YUV420 = 10,
  HWCODEC_CAPTURE_FMT_YVU420 = 11,
  HWCODEC_CAPTURE_FMT_RGB24 = 12,
  HWCODEC_CAPTURE_FMT_BGR24 = 13,
  HWCODEC_CAPTURE_FMT_GREY = 14,
};

typedef struct HwcodecCaptureStreamInfo {
  int width;
  int height;
  int pixel_format;
  int stride;
} HwcodecCaptureStreamInfo;

const char* hwcodec_capture_last_error(void);
char* hwcodec_dshow_list_video_devices(void);
char* hwcodec_dshow_list_device_capabilities(const char* device_name);
void hwcodec_capture_string_free(char* ptr);

HwcodecDshowCaptureContext* hwcodec_dshow_capture_open(
    const char* device_name,
    int width,
    int height,
    int fps,
    int requested_format,
    int timeout_ms);
int hwcodec_dshow_capture_info(
    HwcodecDshowCaptureContext* ctx,
    HwcodecCaptureStreamInfo* out_info);
int hwcodec_dshow_capture_read(
    HwcodecDshowCaptureContext* ctx,
    uint8_t** out_data,
    int* out_len,
    uint64_t* out_sequence);
void hwcodec_dshow_capture_packet_free(uint8_t* data);
void hwcodec_dshow_capture_close(HwcodecDshowCaptureContext* ctx);

#ifdef __cplusplus
}
#endif

#endif
