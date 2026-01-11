// Minimal FFmpeg RAM MJPEG decoder (RKMPP only) -> NV12 in CPU memory.

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavutil/error.h>
#include <libavutil/hwcontext.h>
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
}

#include <string>
#include <string.h>
#include <vector>

#include "common.h"

#define LOG_MODULE "FFMPEG_RAM_DEC"
#include <log.h>
#include <util.h>

typedef void (*RamDecodeCallback)(const uint8_t *data, int len, int width,
                                  int height, int pixfmt, const void *obj);

namespace {
thread_local std::string g_last_error;

static void set_last_error(const std::string &msg) {
  g_last_error = msg;
}

class FFmpegRamDecoder {
public:
  AVCodecContext *c_ = NULL;
  AVPacket *pkt_ = NULL;
  AVFrame *frame_ = NULL;
  AVFrame *sw_frame_ = NULL;
  std::string name_;
  int width_ = 0;
  int height_ = 0;
  AVPixelFormat sw_pixfmt_ = AV_PIX_FMT_NV12;
  int thread_count_ = 1;
  RamDecodeCallback callback_ = NULL;

  AVHWDeviceType hw_device_type_ = AV_HWDEVICE_TYPE_NONE;
  AVPixelFormat hw_pixfmt_ = AV_PIX_FMT_NONE;
  AVBufferRef *hw_device_ctx_ = NULL;

  explicit FFmpegRamDecoder(const char *name, int width, int height, int sw_pixfmt,
                            int thread_count, RamDecodeCallback callback) {
    name_ = name ? name : "";
    width_ = width;
    height_ = height;
    sw_pixfmt_ = (AVPixelFormat)sw_pixfmt;
    thread_count_ = thread_count > 0 ? thread_count : 1;
    callback_ = callback;

    if (name_.find("rkmpp") != std::string::npos) {
      hw_device_type_ = AV_HWDEVICE_TYPE_RKMPP;
    }
  }

  ~FFmpegRamDecoder() {}

  static enum AVPixelFormat get_hw_format(AVCodecContext *ctx,
                                          const enum AVPixelFormat *pix_fmts) {
    FFmpegRamDecoder *dec = reinterpret_cast<FFmpegRamDecoder *>(ctx->opaque);
    if (dec && dec->hw_pixfmt_ != AV_PIX_FMT_NONE) {
      const enum AVPixelFormat *p;
      for (p = pix_fmts; *p != AV_PIX_FMT_NONE; p++) {
        if (*p == dec->hw_pixfmt_) {
          return *p;
        }
      }
    }
    return pix_fmts[0];
  }

  bool init() {
    g_last_error.clear();
    const AVCodec *codec = NULL;
    int ret = 0;

    if (!(codec = avcodec_find_decoder_by_name(name_.c_str()))) {
      set_last_error(std::string("Decoder not found: ") + name_);
      return false;
    }

    if (!(c_ = avcodec_alloc_context3(codec))) {
      set_last_error(std::string("Could not allocate decoder context"));
      return false;
    }

    c_->width = width_;
    c_->height = height_;
    c_->thread_count = thread_count_;
    c_->opaque = this;

    if (hw_device_type_ != AV_HWDEVICE_TYPE_NONE) {
      const AVCodecHWConfig *cfg = NULL;
      for (int i = 0; (cfg = avcodec_get_hw_config(codec, i)); i++) {
        if (cfg->device_type == hw_device_type_) {
          hw_pixfmt_ = cfg->pix_fmt;
          break;
        }
      }
      if (hw_pixfmt_ == AV_PIX_FMT_NONE) {
        set_last_error(std::string("No suitable HW pixfmt for decoder"));
        return false;
      }

      ret = av_hwdevice_ctx_create(&hw_device_ctx_, hw_device_type_, NULL, NULL, 0);
      if (ret < 0) {
        set_last_error(std::string("av_hwdevice_ctx_create failed, ret = ") + av_err2str(ret));
        return false;
      }
      c_->hw_device_ctx = av_buffer_ref(hw_device_ctx_);
      c_->get_format = get_hw_format;

      AVBufferRef *frames_ref = av_hwframe_ctx_alloc(c_->hw_device_ctx);
      if (!frames_ref) {
        set_last_error(std::string("av_hwframe_ctx_alloc failed"));
        return false;
      }
      AVHWFramesContext *frames_ctx = (AVHWFramesContext *)frames_ref->data;
      frames_ctx->format = hw_pixfmt_;
      frames_ctx->sw_format = sw_pixfmt_;
      frames_ctx->width = width_;
      frames_ctx->height = height_;
      frames_ctx->initial_pool_size = 8;
      ret = av_hwframe_ctx_init(frames_ref);
      if (ret < 0) {
        av_buffer_unref(&frames_ref);
        set_last_error(std::string("av_hwframe_ctx_init failed, ret = ") + av_err2str(ret));
        return false;
      }
      c_->hw_frames_ctx = av_buffer_ref(frames_ref);
      av_buffer_unref(&frames_ref);
    }

    if ((ret = avcodec_open2(c_, codec, NULL)) < 0) {
      set_last_error(std::string("avcodec_open2 failed, ret = ") + av_err2str(ret));
      return false;
    }

    pkt_ = av_packet_alloc();
    frame_ = av_frame_alloc();
    sw_frame_ = av_frame_alloc();
    if (!pkt_ || !frame_ || !sw_frame_) {
      set_last_error(std::string("Failed to allocate packet/frame"));
      return false;
    }

    return true;
  }

  int decode(const uint8_t *data, int length, const void *obj) {
    g_last_error.clear();
    int ret = 0;
    if (!c_ || !pkt_ || !frame_) {
      set_last_error(std::string("Decoder not initialized"));
      return -1;
    }

    av_packet_unref(pkt_);
    ret = av_new_packet(pkt_, length);
    if (ret < 0) {
      set_last_error(std::string("av_new_packet failed, ret = ") + av_err2str(ret));
      return ret;
    }
    memcpy(pkt_->data, data, length);
    pkt_->size = length;

    ret = avcodec_send_packet(c_, pkt_);
    av_packet_unref(pkt_);
    if (ret < 0) {
      set_last_error(std::string("avcodec_send_packet failed, ret = ") + av_err2str(ret));
      return ret;
    }

    while (true) {
      ret = avcodec_receive_frame(c_, frame_);
      if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
        break;
      }
      if (ret < 0) {
        set_last_error(std::string("avcodec_receive_frame failed, ret = ") + av_err2str(ret));
        return ret;
      }

      AVFrame *out = frame_;
      if (frame_->format == hw_pixfmt_) {
        av_frame_unref(sw_frame_);
        ret = av_hwframe_transfer_data(sw_frame_, frame_, 0);
        if (ret < 0) {
          set_last_error(std::string("av_hwframe_transfer_data failed, ret = ") + av_err2str(ret));
          return ret;
        }
        out = sw_frame_;
      }

      int buf_size =
          av_image_get_buffer_size((AVPixelFormat)out->format, out->width, out->height, 1);
      if (buf_size < 0) {
        set_last_error(std::string("av_image_get_buffer_size failed, ret = ") + av_err2str(buf_size));
        return buf_size;
      }

      std::vector<uint8_t> buf(buf_size);
      ret = av_image_copy_to_buffer(buf.data(), buf_size,
                                    (const uint8_t *const *)out->data, out->linesize,
                                    (AVPixelFormat)out->format, out->width, out->height, 1);
      if (ret < 0) {
        set_last_error(std::string("av_image_copy_to_buffer failed, ret = ") + av_err2str(ret));
        return ret;
      }

      if (callback_) {
        callback_(buf.data(), buf_size, out->width, out->height, out->format, obj);
      }

      av_frame_unref(frame_);
    }

    return 0;
  }

  void fini() {
    if (pkt_) {
      av_packet_free(&pkt_);
    }
    if (frame_) {
      av_frame_free(&frame_);
    }
    if (sw_frame_) {
      av_frame_free(&sw_frame_);
    }
    if (c_) {
      avcodec_free_context(&c_);
    }
    if (hw_device_ctx_) {
      av_buffer_unref(&hw_device_ctx_);
    }
  }
};
} // namespace

extern "C" void *ffmpeg_ram_new_decoder(const char *name, int width, int height,
                                        int sw_pixfmt, int thread_count,
                                        RamDecodeCallback callback) {
  FFmpegRamDecoder *dec =
      new FFmpegRamDecoder(name, width, height, sw_pixfmt, thread_count, callback);
  if (!dec->init()) {
    dec->fini();
    delete dec;
    return NULL;
  }
  return dec;
}

extern "C" int ffmpeg_ram_decode(void *decoder, const uint8_t *data, int length,
                                 const void *obj) {
  FFmpegRamDecoder *dec = reinterpret_cast<FFmpegRamDecoder *>(decoder);
  if (!dec) {
    return -1;
  }
  return dec->decode(data, length, obj);
}

extern "C" void ffmpeg_ram_free_decoder(void *decoder) {
  FFmpegRamDecoder *dec = reinterpret_cast<FFmpegRamDecoder *>(decoder);
  if (!dec) {
    return;
  }
  dec->fini();
  delete dec;
}

extern "C" const char *ffmpeg_ram_last_error(void) {
  return g_last_error.c_str();
}
