#ifndef FFMPEG_RAM_FFI_H
#define FFMPEG_RAM_FFI_H

#include <stdint.h>

#define AV_NUM_DATA_POINTERS 8

typedef void (*RamEncodeCallback)(const uint8_t *data, int len, int64_t pts,
                                  int key, const void *obj);
typedef void (*RamDecodeCallback)(const uint8_t *data, int len, int width,
                                  int height, int pixfmt, const void *obj);

void *ffmpeg_ram_new_encoder(const char *name, const char *mc_name, int width,
                             int height, int pixfmt, int align, int fps,
                             int gop, int rc, int quality, int kbs, int q,
                             int thread_count, int gpu, int *linesize,
                             int *offset, int *length,
                             RamEncodeCallback callback);
int ffmpeg_ram_encode(void *encoder, const uint8_t *data, int length,
                      const void *obj, int64_t ms);
void ffmpeg_ram_free_encoder(void *encoder);
int ffmpeg_ram_get_linesize_offset_length(int pix_fmt, int width, int height,
                                          int align, int *linesize, int *offset,
                                          int *length);
int ffmpeg_ram_set_bitrate(void *encoder, int kbs);
void ffmpeg_ram_request_keyframe(void *encoder);

void *ffmpeg_ram_new_decoder(const char *name, int width, int height,
                             int sw_pixfmt, int thread_count,
                             RamDecodeCallback callback);
int ffmpeg_ram_decode(void *decoder, const uint8_t *data, int length,
                      const void *obj);
void ffmpeg_ram_free_decoder(void *decoder);
const char *ffmpeg_ram_last_error(void);

#endif // FFMPEG_RAM_FFI_H
