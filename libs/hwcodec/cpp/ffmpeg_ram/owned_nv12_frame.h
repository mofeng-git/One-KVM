#ifndef OWNED_NV12_FRAME_H
#define OWNED_NV12_FRAME_H

extern "C" {
#include <libavutil/frame.h>
}

#include <limits>
#include <memory>

using RamInputRelease = void (*)(void *, uint8_t *);

struct RamFrameDeleter {
  void operator()(AVFrame *frame) const { av_frame_free(&frame); }
};

using RamOwnedFrame = std::unique_ptr<AVFrame, RamFrameDeleter>;

static RamOwnedFrame make_owned_nv12_frame(const uint8_t *data, int length,
                                            int width, int height, void *owner,
                                            RamInputRelease release) {
  const int64_t pixels = static_cast<int64_t>(width) * height;
  if (!data || !release || width <= 0 || height <= 0 || width % 2 ||
      height % 2 || pixels > std::numeric_limits<int>::max() / 3 * 2 ||
      length < pixels + pixels / 2) {
    if (release)
      release(owner, const_cast<uint8_t *>(data));
    return RamOwnedFrame();
  }
  RamOwnedFrame frame(av_frame_alloc());
  if (!frame) {
    release(owner, const_cast<uint8_t *>(data));
    return frame;
  }
  frame->buf[0] = av_buffer_create(const_cast<uint8_t *>(data), length, release,
                                  owner, AV_BUFFER_FLAG_READONLY);
  if (!frame->buf[0]) {
    release(owner, const_cast<uint8_t *>(data));
    return RamOwnedFrame();
  }
  frame->format = AV_PIX_FMT_NV12;
  frame->width = width;
  frame->height = height;
  frame->data[0] = frame->buf[0]->data;
  frame->data[1] = frame->data[0] + pixels;
  frame->linesize[0] = width;
  frame->linesize[1] = width;
  return frame;
}

#endif
