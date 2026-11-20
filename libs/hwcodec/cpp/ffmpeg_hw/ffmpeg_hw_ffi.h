#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// MJPEG -> H26x (H.264 / H.265) hardware pipeline
typedef struct FfmpegHwMjpegH26x FfmpegHwMjpegH26x;
typedef struct FfmpegHwMjpegInput FfmpegHwMjpegInput;
typedef void (*FfmpegHwMjpegWakeup)(void *user);

// Create a new MJPEG -> H26x pipeline.
FfmpegHwMjpegH26x* ffmpeg_hw_mjpeg_h26x_new(const char* dec_name,
                                            const char* enc_name,
                                            int width,
                                            int height,
                                            int fps,
                                            int bitrate_kbps,
                                            int gop,
                                            int thread_count);

// Submit MJPEG and return any encoded packet with its original timestamp.
// Returns 1 if output produced, 0 if no output, <0 on error.
int ffmpeg_hw_mjpeg_h26x_encode(FfmpegHwMjpegH26x* ctx,
                                const uint8_t* data,
                                int len,
                                int64_t pts_ms,
                                uint8_t** out_data,
                                int* out_len,
                                int* out_keyframe,
                                int64_t* out_pts_ms);

// The wakeup runs on the decode worker and stays valid until the pipeline is freed.
FfmpegHwMjpegInput *ffmpeg_hw_mjpeg_h26x_start_worker(FfmpegHwMjpegH26x *ctx,
                                                     FfmpegHwMjpegWakeup wakeup, void *user);
// The capture thread owns this input handle. It may outlive the encoder.
int ffmpeg_hw_mjpeg_h26x_submit(FfmpegHwMjpegInput *input, const uint8_t *data,
                               int len, int64_t pts_ms);
void ffmpeg_hw_mjpeg_h26x_free_input(FfmpegHwMjpegInput *input);
int ffmpeg_hw_mjpeg_h26x_ready(FfmpegHwMjpegH26x *ctx);
void ffmpeg_hw_mjpeg_h26x_discard_ready(FfmpegHwMjpegH26x *ctx);
int ffmpeg_hw_mjpeg_h26x_encode_ready(FfmpegHwMjpegH26x *ctx,
                                    uint8_t **out_data, int *out_len,
                                    int *out_keyframe, int64_t *out_pts_ms);

// Reconfigure bitrate/gop (best-effort, may recreate encoder internally).
int ffmpeg_hw_mjpeg_h26x_reconfigure(FfmpegHwMjpegH26x* ctx,
                                     int bitrate_kbps,
                                     int gop);

// Request next frame to be a keyframe.
int ffmpeg_hw_mjpeg_h26x_request_keyframe(FfmpegHwMjpegH26x* ctx);

// Free pipeline resources.
void ffmpeg_hw_mjpeg_h26x_free(FfmpegHwMjpegH26x* ctx);

// Free packet buffer allocated by ffmpeg_hw_mjpeg_h26x_encode.
void ffmpeg_hw_packet_free(uint8_t* data);

// Get last error message (thread-local).
const char* ffmpeg_hw_last_error(void);

#ifdef __cplusplus
}
#endif
