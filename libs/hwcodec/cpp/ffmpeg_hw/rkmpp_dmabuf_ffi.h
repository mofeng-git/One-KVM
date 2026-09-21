#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct RkmppDmaEncoder RkmppDmaEncoder;
// format: 0=NV12, 1=BGR24, 2=YUYV, 3=RGB24 (byte stride), 4=MJPEG (stride ignored).
// codec: 0=H264, 1=HEVC. MJPEG is decoded to hardware NV12, then encoded.
RkmppDmaEncoder *rkmpp_dma_new(int width, int height, int stride, int format,
                              int codec, int fps, int kbps, int gop,
                              const int *fds, const size_t *sizes, size_t count);
// Synchronous input-completion boundary. On failure the encoder is destroyed
// internally BEFORE returning, so it cannot keep reading the capture buffer.
// Output is borrowed until the next call or destruction; copy before reusing it.
// bytes_used must be the actual captured payload. MJPEG needs 64 bytes of
// readable allocation headroom; never pass sizeimage as the compressed length.
// fresh_fd=-1 reuses the original import (native HDMI). UVC supplies a new
// export of this dequeued slot. Keep it open until replacement/free; the old
// export can be closed AFTER this call, including on failure.
int rkmpp_dma_encode(RkmppDmaEncoder *encoder, size_t index, size_t bytes_used, int fresh_fd, int64_t pts_us,
                     int force_idr, const uint8_t **data, size_t *size);
int rkmpp_dma_reconfigure(RkmppDmaEncoder *encoder, int kbps, int gop);
void rkmpp_dma_free(RkmppDmaEncoder *encoder);
const char *rkmpp_dma_error(void);

#ifdef __cplusplus
}
#endif
