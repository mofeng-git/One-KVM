extern "C" {
#include <libavcodec/avcodec.h>
#include <libavutil/avutil.h>
#include <libavutil/error.h>
#include <libavutil/hwcontext.h>
#include <libavutil/hwcontext_drm.h>
#include <libavutil/pixdesc.h>
#include <libavutil/opt.h>
}

#include <string>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

#define LOG_MODULE "FFMPEG_HW"
#include "../common/log.h"

#include "ffmpeg_hw_ffi.h"

namespace {
thread_local std::string g_last_error;

static void set_last_error(const std::string &msg) {
  g_last_error = msg;
  LOG_ERROR(msg);
}

static std::string make_err(const std::string &ctx, int err) {
  return ctx + " (ret=" + std::to_string(err) + "): " + av_err2str(err);
}

static const char* pix_fmt_name(AVPixelFormat fmt) {
  const char *name = av_get_pix_fmt_name(fmt);
  return name ? name : "unknown";
}

struct FfmpegHwMjpegH26xCtx {
  AVCodecContext *dec_ctx = nullptr;
  AVCodecContext *enc_ctx = nullptr;
  AVPacket *dec_pkt = nullptr;
  AVFrame *dec_frame = nullptr;
  AVPacket *enc_pkt = nullptr;
  AVBufferRef *hw_device_ctx = nullptr;
  AVBufferRef *hw_frames_ctx = nullptr;
  AVPixelFormat hw_pixfmt = AV_PIX_FMT_NONE;
  std::string dec_name;
  std::string enc_name;
  int width = 0;
  int height = 0;
  int aligned_width = 0;
  int aligned_height = 0;
  int fps = 30;
  int bitrate_kbps = 2000;
  int gop = 60;
  int thread_count = 1;
  bool force_keyframe = false;
};

static enum AVPixelFormat get_hw_format(AVCodecContext *ctx,
                                        const enum AVPixelFormat *pix_fmts) {
  auto *self = reinterpret_cast<FfmpegHwMjpegH26xCtx *>(ctx->opaque);
  if (self && self->hw_pixfmt != AV_PIX_FMT_NONE) {
    const enum AVPixelFormat *p;
    for (p = pix_fmts; *p != AV_PIX_FMT_NONE; p++) {
      if (*p == self->hw_pixfmt) {
        return *p;
      }
    }
  }
  return pix_fmts[0];
}

static int init_decoder(FfmpegHwMjpegH26xCtx *ctx) {
  const AVCodec *dec = avcodec_find_decoder_by_name(ctx->dec_name.c_str());
  if (!dec) {
    set_last_error("Decoder not found: " + ctx->dec_name);
    return -1;
  }

  ctx->dec_ctx = avcodec_alloc_context3(dec);
  if (!ctx->dec_ctx) {
    set_last_error("Failed to allocate decoder context");
    return -1;
  }

  ctx->dec_ctx->width = ctx->width;
  ctx->dec_ctx->height = ctx->height;
  ctx->dec_ctx->thread_count = ctx->thread_count > 0 ? ctx->thread_count : 1;
  ctx->dec_ctx->opaque = ctx;

  // Pick HW pixfmt for RKMPP
  const AVCodecHWConfig *cfg = nullptr;
  for (int i = 0; (cfg = avcodec_get_hw_config(dec, i)); i++) {
    if (cfg->device_type == AV_HWDEVICE_TYPE_RKMPP) {
      ctx->hw_pixfmt = cfg->pix_fmt;
      break;
    }
  }
  if (ctx->hw_pixfmt == AV_PIX_FMT_NONE) {
    set_last_error("No RKMPP hw pixfmt for decoder");
    return -1;
  }

  int ret = av_hwdevice_ctx_create(&ctx->hw_device_ctx,
                                   AV_HWDEVICE_TYPE_RKMPP, NULL, NULL, 0);
  if (ret < 0) {
    set_last_error(make_err("av_hwdevice_ctx_create failed", ret));
    return -1;
  }

  ctx->dec_ctx->hw_device_ctx = av_buffer_ref(ctx->hw_device_ctx);
  ctx->dec_ctx->get_format = get_hw_format;

  ret = avcodec_open2(ctx->dec_ctx, dec, NULL);
  if (ret < 0) {
    set_last_error(make_err("avcodec_open2 decoder failed", ret));
    return -1;
  }

  ctx->dec_pkt = av_packet_alloc();
  ctx->dec_frame = av_frame_alloc();
  ctx->enc_pkt = av_packet_alloc();
  if (!ctx->dec_pkt || !ctx->dec_frame || !ctx->enc_pkt) {
    set_last_error("Failed to allocate packet/frame");
    return -1;
  }

  return 0;
}

static int init_encoder(FfmpegHwMjpegH26xCtx *ctx, AVBufferRef *frames_ctx) {
  const AVCodec *enc = avcodec_find_encoder_by_name(ctx->enc_name.c_str());
  if (!enc) {
    set_last_error("Encoder not found: " + ctx->enc_name);
    return -1;
  }

  ctx->enc_ctx = avcodec_alloc_context3(enc);
  if (!ctx->enc_ctx) {
    set_last_error("Failed to allocate encoder context");
    return -1;
  }

  ctx->enc_ctx->width = ctx->width;
  ctx->enc_ctx->height = ctx->height;
  ctx->enc_ctx->coded_width = ctx->width;
  ctx->enc_ctx->coded_height = ctx->height;
  ctx->aligned_width = ctx->width;
  ctx->aligned_height = ctx->height;
  ctx->enc_ctx->time_base = AVRational{1, 1000};
  ctx->enc_ctx->framerate = AVRational{ctx->fps, 1};
  ctx->enc_ctx->bit_rate = (int64_t)ctx->bitrate_kbps * 1000;
  ctx->enc_ctx->gop_size = ctx->gop > 0 ? ctx->gop : ctx->fps;
  ctx->enc_ctx->max_b_frames = 0;
  ctx->enc_ctx->pix_fmt = AV_PIX_FMT_DRM_PRIME;
  ctx->enc_ctx->sw_pix_fmt = AV_PIX_FMT_NV12;

  if (frames_ctx) {
    AVHWFramesContext *hwfc = reinterpret_cast<AVHWFramesContext *>(frames_ctx->data);
    if (hwfc) {
      ctx->enc_ctx->pix_fmt = static_cast<AVPixelFormat>(hwfc->format);
      ctx->enc_ctx->sw_pix_fmt = static_cast<AVPixelFormat>(hwfc->sw_format);
      if (hwfc->width > 0) {
        ctx->aligned_width = hwfc->width;
        ctx->enc_ctx->coded_width = hwfc->width;
      }
      if (hwfc->height > 0) {
        ctx->aligned_height = hwfc->height;
        ctx->enc_ctx->coded_height = hwfc->height;
      }
    }
    ctx->hw_frames_ctx = av_buffer_ref(frames_ctx);
    ctx->enc_ctx->hw_frames_ctx = av_buffer_ref(frames_ctx);
  }
  if (ctx->hw_device_ctx) {
    ctx->enc_ctx->hw_device_ctx = av_buffer_ref(ctx->hw_device_ctx);
  }

  AVDictionary *opts = nullptr;
  av_dict_set(&opts, "rc_mode", "CBR", 0);
  if (enc->id == AV_CODEC_ID_H264) {
    av_dict_set(&opts, "profile", "high", 0);
  } else if (enc->id == AV_CODEC_ID_HEVC) {
    av_dict_set(&opts, "profile", "main", 0);
  }
  av_dict_set_int(&opts, "qp_init", 23, 0);
  av_dict_set_int(&opts, "qp_max", 48, 0);
  av_dict_set_int(&opts, "qp_min", 0, 0);
  av_dict_set_int(&opts, "qp_max_i", 48, 0);
  av_dict_set_int(&opts, "qp_min_i", 0, 0);
  int ret = avcodec_open2(ctx->enc_ctx, enc, &opts);
  av_dict_free(&opts);
  if (ret < 0) {
    std::string detail = "avcodec_open2 encoder failed: ";
    detail += ctx->enc_name;
    detail += " fmt=" + std::string(pix_fmt_name(ctx->enc_ctx->pix_fmt));
    detail += " sw=" + std::string(pix_fmt_name(ctx->enc_ctx->sw_pix_fmt));
    detail += " size=" + std::to_string(ctx->enc_ctx->width) + "x" + std::to_string(ctx->enc_ctx->height);
    detail += " fps=" + std::to_string(ctx->fps);
    set_last_error(make_err(detail, ret));
    avcodec_free_context(&ctx->enc_ctx);
    ctx->enc_ctx = nullptr;
    if (ctx->hw_frames_ctx) {
      av_buffer_unref(&ctx->hw_frames_ctx);
      ctx->hw_frames_ctx = nullptr;
    }
    return -1;
  }

  return 0;
}

static void free_encoder(FfmpegHwMjpegH26xCtx *ctx) {
  if (ctx->enc_ctx) {
    avcodec_free_context(&ctx->enc_ctx);
    ctx->enc_ctx = nullptr;
  }
  if (ctx->hw_frames_ctx) {
    av_buffer_unref(&ctx->hw_frames_ctx);
    ctx->hw_frames_ctx = nullptr;
  }
}

} // namespace

extern "C" FfmpegHwMjpegH26x* ffmpeg_hw_mjpeg_h26x_new(const char* dec_name,
                                                        const char* enc_name,
                                                        int width,
                                                        int height,
                                                        int fps,
                                                        int bitrate_kbps,
                                                        int gop,
                                                        int thread_count) {
  if (!dec_name || !enc_name || width <= 0 || height <= 0) {
    set_last_error("Invalid parameters for ffmpeg_hw_mjpeg_h26x_new");
    return nullptr;
  }

  auto *ctx = new FfmpegHwMjpegH26xCtx();
  ctx->dec_name = dec_name;
  ctx->enc_name = enc_name;
  ctx->width = width;
  ctx->height = height;
  ctx->fps = fps > 0 ? fps : 30;
  ctx->bitrate_kbps = bitrate_kbps > 0 ? bitrate_kbps : 2000;
  ctx->gop = gop > 0 ? gop : ctx->fps;
  ctx->thread_count = thread_count > 0 ? thread_count : 1;

  if (init_decoder(ctx) != 0) {
    ffmpeg_hw_mjpeg_h26x_free(reinterpret_cast<FfmpegHwMjpegH26x*>(ctx));
    return nullptr;
  }

  return reinterpret_cast<FfmpegHwMjpegH26x*>(ctx);
}

extern "C" int ffmpeg_hw_mjpeg_h26x_encode(FfmpegHwMjpegH26x* handle,
                                             const uint8_t* data,
                                             int len,
                                             int64_t pts_ms,
                                             uint8_t** out_data,
                                             int* out_len,
                                             int* out_keyframe) {
  if (!handle || !data || len <= 0 || !out_data || !out_len || !out_keyframe) {
    set_last_error("Invalid parameters for encode");
    return -1;
  }

  auto *ctx = reinterpret_cast<FfmpegHwMjpegH26xCtx*>(handle);
  *out_data = nullptr;
  *out_len = 0;
  *out_keyframe = 0;

  av_packet_unref(ctx->dec_pkt);
  int ret = av_new_packet(ctx->dec_pkt, len);
  if (ret < 0) {
    set_last_error(make_err("av_new_packet failed", ret));
    return -1;
  }
  memcpy(ctx->dec_pkt->data, data, len);
  ctx->dec_pkt->size = len;

  ret = avcodec_send_packet(ctx->dec_ctx, ctx->dec_pkt);
  if (ret < 0) {
    set_last_error(make_err("avcodec_send_packet failed", ret));
    return -1;
  }

  while (true) {
    ret = avcodec_receive_frame(ctx->dec_ctx, ctx->dec_frame);
    if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
      return 0;
    }
    if (ret < 0) {
      set_last_error(make_err("avcodec_receive_frame failed", ret));
      return -1;
    }

    if (ctx->dec_frame->format != AV_PIX_FMT_DRM_PRIME) {
      set_last_error("Decoder output is not DRM_PRIME");
      av_frame_unref(ctx->dec_frame);
      return -1;
    }

    if (!ctx->enc_ctx) {
      if (!ctx->dec_frame->hw_frames_ctx) {
        set_last_error("Decoder returned frame without hw_frames_ctx");
        av_frame_unref(ctx->dec_frame);
        return -1;
      }
      if (init_encoder(ctx, ctx->dec_frame->hw_frames_ctx) != 0) {
        av_frame_unref(ctx->dec_frame);
        return -1;
      }
    }

    AVFrame *send_frame = ctx->dec_frame;
    AVFrame *tmp = nullptr;
    if (ctx->force_keyframe) {
      tmp = av_frame_clone(send_frame);
      if (tmp) {
        tmp->pict_type = AV_PICTURE_TYPE_I;
        send_frame = tmp;
      }
      ctx->force_keyframe = false;
    }

    // Apply visible size crop if aligned buffer is larger than display size
    if (ctx->aligned_width > 0 && ctx->width > 0 && ctx->aligned_width > ctx->width) {
      send_frame->crop_right = ctx->aligned_width - ctx->width;
    }
    if (ctx->aligned_height > 0 && ctx->height > 0 && ctx->aligned_height > ctx->height) {
      send_frame->crop_bottom = ctx->aligned_height - ctx->height;
    }

    send_frame->pts = pts_ms; // time_base is ms

    ret = avcodec_send_frame(ctx->enc_ctx, send_frame);
    if (tmp) {
      av_frame_free(&tmp);
    }
    if (ret < 0) {
      std::string detail = "avcodec_send_frame failed";
      if (send_frame) {
        detail += " frame_fmt=";
        detail += pix_fmt_name(static_cast<AVPixelFormat>(send_frame->format));
        detail += " w=" + std::to_string(send_frame->width);
        detail += " h=" + std::to_string(send_frame->height);
        if (send_frame->format == AV_PIX_FMT_DRM_PRIME && send_frame->data[0]) {
          const AVDRMFrameDescriptor *drm =
              reinterpret_cast<const AVDRMFrameDescriptor *>(send_frame->data[0]);
          if (drm && drm->layers[0].format) {
            detail += " drm_fmt=0x";
            char buf[9];
            snprintf(buf, sizeof(buf), "%08x", drm->layers[0].format);
            detail += buf;
          }
          if (drm && drm->objects[0].format_modifier) {
            detail += " drm_mod=0x";
            char buf[17];
            snprintf(buf, sizeof(buf), "%016llx",
                     (unsigned long long)drm->objects[0].format_modifier);
            detail += buf;
          }
        }
      }
      set_last_error(make_err(detail, ret));
      av_frame_unref(ctx->dec_frame);
      return -1;
    }

    av_packet_unref(ctx->enc_pkt);
    ret = avcodec_receive_packet(ctx->enc_ctx, ctx->enc_pkt);
    if (ret == AVERROR(EAGAIN)) {
      av_frame_unref(ctx->dec_frame);
      return 0;
    }
    if (ret < 0) {
      set_last_error(make_err("avcodec_receive_packet failed", ret));
      av_frame_unref(ctx->dec_frame);
      return -1;
    }

    if (ctx->enc_pkt->size > 0) {
      uint8_t *buf = (uint8_t*)malloc(ctx->enc_pkt->size);
      if (!buf) {
        set_last_error("malloc for output packet failed");
        av_packet_unref(ctx->enc_pkt);
        av_frame_unref(ctx->dec_frame);
        return -1;
      }
      memcpy(buf, ctx->enc_pkt->data, ctx->enc_pkt->size);
      *out_data = buf;
      *out_len = ctx->enc_pkt->size;
      *out_keyframe = (ctx->enc_pkt->flags & AV_PKT_FLAG_KEY) ? 1 : 0;
      av_packet_unref(ctx->enc_pkt);
      av_frame_unref(ctx->dec_frame);
      return 1;
    }

    av_frame_unref(ctx->dec_frame);
  }
}

extern "C" int ffmpeg_hw_mjpeg_h26x_reconfigure(FfmpegHwMjpegH26x* handle,
                                                  int bitrate_kbps,
                                                  int gop) {
  if (!handle) {
    set_last_error("Invalid handle for reconfigure");
    return -1;
  }
  auto *ctx = reinterpret_cast<FfmpegHwMjpegH26xCtx*>(handle);
  if (!ctx->enc_ctx || !ctx->hw_frames_ctx) {
    set_last_error("Encoder not initialized for reconfigure");
    return -1;
  }

  ctx->bitrate_kbps = bitrate_kbps > 0 ? bitrate_kbps : ctx->bitrate_kbps;
  ctx->gop = gop > 0 ? gop : ctx->gop;

  AVBufferRef *frames_ref = ctx->hw_frames_ctx ? av_buffer_ref(ctx->hw_frames_ctx) : nullptr;
  free_encoder(ctx);

  if (init_encoder(ctx, frames_ref) != 0) {
    if (frames_ref) av_buffer_unref(&frames_ref);
    return -1;
  }
  if (frames_ref) av_buffer_unref(&frames_ref);

  return 0;
}

extern "C" int ffmpeg_hw_mjpeg_h26x_request_keyframe(FfmpegHwMjpegH26x* handle) {
  if (!handle) {
    set_last_error("Invalid handle for request_keyframe");
    return -1;
  }
  auto *ctx = reinterpret_cast<FfmpegHwMjpegH26xCtx*>(handle);
  ctx->force_keyframe = true;
  return 0;
}

extern "C" void ffmpeg_hw_mjpeg_h26x_free(FfmpegHwMjpegH26x* handle) {
  auto *ctx = reinterpret_cast<FfmpegHwMjpegH26xCtx*>(handle);
  if (!ctx) return;

  if (ctx->dec_pkt) av_packet_free(&ctx->dec_pkt);
  if (ctx->dec_frame) av_frame_free(&ctx->dec_frame);
  if (ctx->enc_pkt) av_packet_free(&ctx->enc_pkt);

  if (ctx->dec_ctx) avcodec_free_context(&ctx->dec_ctx);
  free_encoder(ctx);

  if (ctx->hw_device_ctx) av_buffer_unref(&ctx->hw_device_ctx);

  delete ctx;
}

extern "C" void ffmpeg_hw_packet_free(uint8_t* data) {
  if (data) {
    free(data);
  }
}

extern "C" const char* ffmpeg_hw_last_error(void) {
  return g_last_error.c_str();
}
