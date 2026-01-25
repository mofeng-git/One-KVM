#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct FfmpegHwMjpegH264 FfmpegHwMjpegH264;

FfmpegHwMjpegH264* ffmpeg_hw_mjpeg_h264_new(const char* dec_name,
                                            const char* enc_name,
                                            int width,
                                            int height,
                                            int fps,
                                            int bitrate_kbps,
                                            int gop,
                                            int thread_count);

int ffmpeg_hw_mjpeg_h264_encode(FfmpegHwMjpegH264* ctx,
                                const uint8_t* data,
                                int len,
                                int64_t pts_ms,
                                uint8_t** out_data,
                                int* out_len,
                                int* out_keyframe);

int ffmpeg_hw_mjpeg_h264_reconfigure(FfmpegHwMjpegH264* ctx,
                                     int bitrate_kbps,
                                     int gop);

int ffmpeg_hw_mjpeg_h264_request_keyframe(FfmpegHwMjpegH264* ctx);

void ffmpeg_hw_mjpeg_h264_free(FfmpegHwMjpegH264* ctx);

void ffmpeg_hw_packet_free(uint8_t* data);

const char* ffmpeg_hw_last_error(void);

#ifdef __cplusplus
}
#endif
