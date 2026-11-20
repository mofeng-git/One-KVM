extern "C" {
#include <libavcodec/avcodec.h>
#include <libavutil/avutil.h>
#include <libavutil/error.h>
#include <libavutil/hwcontext.h>
#include <libavutil/hwcontext_drm.h>
#include <libavutil/pixdesc.h>
#include <libavutil/opt.h>
}

#include <condition_variable>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
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

class MjpegDecodeWorker;

struct JpegColor {
  AVColorRange range = AVCOL_RANGE_JPEG;
  AVColorSpace matrix = AVCOL_SPC_SMPTE170M;
  AVColorPrimaries primaries = AVCOL_PRI_BT709; // sRGB chromaticities
  AVColorTransferCharacteristic transfer = AVCOL_TRC_IEC61966_2_1;

  bool operator!=(const JpegColor &other) const {
    return range != other.range || matrix != other.matrix ||
           primaries != other.primaries || transfer != other.transfer;
  }
};

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
  bool jpeg_output_nv12 = false;
  JpegColor color;
  std::shared_ptr<MjpegDecodeWorker> decode_worker;
};

// Internal result, only returned for a failed submission of native NV16.
constexpr int retry_with_nv12 = -2;

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

  AVDictionary *opts = nullptr;
  if (ctx->jpeg_output_nv12) {
    av_dict_set(&opts, "mjpeg_output_nv12", "1", 0);
  }
  ret = avcodec_open2(ctx->dec_ctx, dec, &opts);
  const bool unsupported_nv12 = av_dict_get(opts, "mjpeg_output_nv12", nullptr, 0);
  av_dict_free(&opts);
  if (ret < 0) {
    set_last_error(make_err("avcodec_open2 decoder failed", ret));
    return -1;
  }
  if (unsupported_nv12) {
    set_last_error("Installed RKMPP JPEG decoder cannot request NV12");
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
  ctx->enc_ctx->flags |= AV_CODEC_FLAG_LOW_DELAY;
  ctx->enc_ctx->pix_fmt = AV_PIX_FMT_DRM_PRIME;
  ctx->enc_ctx->sw_pix_fmt = AV_PIX_FMT_NV12;
  ctx->enc_ctx->color_range = ctx->color.range;
  ctx->enc_ctx->colorspace = ctx->color.matrix;
  ctx->enc_ctx->color_primaries = ctx->color.primaries;
  ctx->enc_ctx->color_trc = ctx->color.transfer;

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

static bool is_nv16(const AVFrame *frame) {
  if (!frame->hw_frames_ctx)
    return false;
  const auto *frames = reinterpret_cast<const AVHWFramesContext *>(frame->hw_frames_ctx->data);
  return frames->sw_format == AV_PIX_FMT_NV16;
}

static int decode_mjpeg_packet(AVCodecContext *decoder, const AVPacket *packet,
                               AVFrame *frame) {
  int ret = avcodec_send_packet(decoder, packet);
  return ret < 0 ? ret : avcodec_receive_frame(decoder, frame);
}

struct MjpegDecodeJob {
  AVPacket *packet = av_packet_alloc();
  AVFrame *frame = av_frame_alloc();
  int result = 0;

  ~MjpegDecodeJob() {
    av_frame_free(&frame);
    av_packet_free(&packet);
  }
};

// One latest JPEG waiting for decode and one latest decoded frame waiting
// for encode. Codec calls and the application wakeup run outside the lock.
class MjpegDecodeWorker {
 public:
  MjpegDecodeWorker(AVCodecContext *decoder, FfmpegHwMjpegWakeup wakeup, void *user)
      : decoder_(decoder), wakeup_(wakeup), user_(user),
        thread_([this] { run(); }) {}

  ~MjpegDecodeWorker() { stop(); }

  void stop() {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      stop_ = true;
      input_.reset();
    }
    ready_.notify_one();
    if (thread_.joinable())
      thread_.join();
    output_.reset();
  }

  bool submit(std::unique_ptr<MjpegDecodeJob> job) {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      if (stop_)
        return false;
      input_ = std::move(job);
    }
    ready_.notify_one();
    return true;
  }

  bool has_output() {
    std::lock_guard<std::mutex> lock(mutex_);
    return output_ != nullptr;
  }

  std::unique_ptr<MjpegDecodeJob> take_output() {
    std::lock_guard<std::mutex> lock(mutex_);
    return std::move(output_);
  }

  std::unique_ptr<MjpegDecodeJob> pause() {
    std::unique_lock<std::mutex> lock(mutex_);
    paused_ = true;
    idle_.wait(lock, [this] { return !decoding_; });
    auto latest = std::move(input_);
    if (!latest)
      latest = std::move(output_);
    output_.reset();
    if (latest)
      av_frame_unref(latest->frame);
    return latest;
  }

  void resume(AVCodecContext *decoder, std::unique_ptr<MjpegDecodeJob> latest) {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      decoder_ = decoder;
      if (!input_)
        input_ = std::move(latest);
      paused_ = false;
    }
    ready_.notify_one();
  }

 private:
  void run() {
    std::unique_lock<std::mutex> lock(mutex_);
    while (true) {
      ready_.wait(lock, [this] { return stop_ || (!paused_ && input_); });
      if (stop_)
        return;

      auto job = std::move(input_);
      auto *decoder = decoder_;
      decoding_ = true;
      lock.unlock();
      job->result = decode_mjpeg_packet(decoder, job->packet, job->frame);
      lock.lock();
      output_ = std::move(job);
      decoding_ = false;
      idle_.notify_one();
      lock.unlock();
      wakeup_(user_);
      lock.lock();
    }
  }

  std::mutex mutex_;
  std::condition_variable ready_;
  std::condition_variable idle_;
  std::unique_ptr<MjpegDecodeJob> input_;
  std::unique_ptr<MjpegDecodeJob> output_;
  AVCodecContext *decoder_;
  FfmpegHwMjpegWakeup wakeup_;
  void *user_;
  bool decoding_ = false;
  bool paused_ = false;
  bool stop_ = false;
  std::thread thread_;
};

static void free_pipeline(FfmpegHwMjpegH26xCtx *ctx) {
  if (ctx->decode_worker)
    ctx->decode_worker->stop();
  ctx->decode_worker.reset();
  av_packet_free(&ctx->dec_pkt);
  av_frame_free(&ctx->dec_frame);
  av_packet_free(&ctx->enc_pkt);
  // Release all encoder references to decoder frames before closing MPP.
  free_encoder(ctx);
  avcodec_free_context(&ctx->dec_ctx);
  av_buffer_unref(&ctx->hw_device_ctx);
  ctx->hw_pixfmt = AV_PIX_FMT_NONE;
}

} // namespace

struct FfmpegHwMjpegInput {
  std::shared_ptr<MjpegDecodeWorker> worker;
};

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

static int prepare_packet(AVPacket *packet, const uint8_t *data, int len,
                          int64_t pts_ms) {
  av_packet_unref(packet);
  const int ret = av_new_packet(packet, len);
  if (ret < 0) {
    set_last_error(make_err("av_new_packet failed", ret));
    return -1;
  }
  memcpy(packet->data, data, len);
  packet->pts = pts_ms;
  return 0;
}

static int encode_decoded_frame(FfmpegHwMjpegH26xCtx *ctx, AVFrame *frame,
                                int64_t pts_ms, uint8_t **out_data, int *out_len,
                                int *out_keyframe, int64_t *out_pts_ms) {
  if (frame->format != AV_PIX_FMT_DRM_PRIME || !frame->hw_frames_ctx) {
    set_last_error("Decoder output has no DRM_PRIME frame context");
    return -1;
  }

  // V4L2's JPEG defaults are full-range BT.601 with sRGB primaries/transfer.
  // The NV16 -> NV12 retry changes chroma sampling without remapping levels.
  JpegColor color;
  if (frame->color_range != AVCOL_RANGE_UNSPECIFIED)
    color.range = frame->color_range;
  if (frame->colorspace != AVCOL_SPC_UNSPECIFIED &&
      frame->colorspace != AVCOL_SPC_RESERVED && frame->colorspace != AVCOL_SPC_RGB)
    color.matrix = frame->colorspace;
  if (frame->color_primaries != AVCOL_PRI_UNSPECIFIED &&
      frame->color_primaries != AVCOL_PRI_RESERVED0 && frame->color_primaries != AVCOL_PRI_RESERVED)
    color.primaries = frame->color_primaries;
  if (frame->color_trc != AVCOL_TRC_UNSPECIFIED &&
      frame->color_trc != AVCOL_TRC_RESERVED0 && frame->color_trc != AVCOL_TRC_RESERVED)
    color.transfer = frame->color_trc;
  if (ctx->enc_ctx && ctx->color != color) {
    free_encoder(ctx);
    ctx->force_keyframe = true;
  }
  ctx->color = color;
  if (!ctx->enc_ctx && init_encoder(ctx, frame->hw_frames_ctx) != 0)
    return is_nv16(frame) ? retry_with_nv12 : -1;

  if (ctx->force_keyframe) {
    frame->pict_type = AV_PICTURE_TYPE_I;
    ctx->force_keyframe = false;
  }
  if (ctx->aligned_width > ctx->width)
    frame->crop_right = ctx->aligned_width - ctx->width;
  if (ctx->aligned_height > ctx->height)
    frame->crop_bottom = ctx->aligned_height - ctx->height;
  frame->pts = pts_ms;

  int ret = avcodec_send_frame(ctx->enc_ctx, frame);
  if (ret < 0) {
    set_last_error(make_err("avcodec_send_frame failed", ret));
    return ret != AVERROR(EAGAIN) && is_nv16(frame) ? retry_with_nv12 : -1;
  }

  while (true) {
    av_packet_unref(ctx->enc_pkt);
    ret = avcodec_receive_packet(ctx->enc_ctx, ctx->enc_pkt);
    if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF)
      return *out_data ? 1 : 0;
    if (ret < 0) {
      set_last_error(make_err("avcodec_receive_packet failed", ret));
      return is_nv16(frame) ? retry_with_nv12 : -1;
    }
    if (ctx->enc_pkt->size <= 0) {
      set_last_error("avcodec_receive_packet failed, pkt size is 0");
      return -1;
    }
    if (*out_data)
      continue;

    auto *buf = static_cast<uint8_t *>(malloc(ctx->enc_pkt->size));
    if (!buf) {
      set_last_error("malloc for output packet failed");
      return -1;
    }
    memcpy(buf, ctx->enc_pkt->data, ctx->enc_pkt->size);
    *out_data = buf;
    *out_len = ctx->enc_pkt->size;
    *out_keyframe = (ctx->enc_pkt->flags & AV_PKT_FLAG_KEY) != 0;
    *out_pts_ms = ctx->enc_pkt->pts == AV_NOPTS_VALUE ? pts_ms : ctx->enc_pkt->pts;
  }
}

static int encode_mjpeg_packet(FfmpegHwMjpegH26xCtx *ctx, const AVPacket *packet,
                               uint8_t **out_data, int *out_len, int *out_keyframe,
                               int64_t *out_pts_ms) {
  const int ret = decode_mjpeg_packet(ctx->dec_ctx, packet, ctx->dec_frame);
  if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF)
    return 0;
  if (ret < 0) {
    set_last_error(make_err("RKMPP MJPEG decode failed", ret));
    return -1;
  }
  const int encoded = encode_decoded_frame(ctx, ctx->dec_frame, packet->pts,
                                          out_data, out_len, out_keyframe,
                                          out_pts_ms);
  av_frame_unref(ctx->dec_frame);
  return encoded;
}

static int finish_mjpeg_encode(FfmpegHwMjpegH26xCtx *ctx, int ret,
                               AVPacket *jpeg, uint8_t **out_data, int *out_len,
                               int *out_keyframe, int64_t *out_pts_ms) {
  if (ret >= 0)
    return ret;
  free(*out_data);
  *out_data = nullptr;
  *out_len = 0;
  *out_keyframe = 0;
  if (ret != retry_with_nv12 || ctx->jpeg_output_nv12)
    return -1;

  AVPacket *retry = av_packet_clone(jpeg);
  if (!retry) {
    set_last_error("Failed to retain JPEG for RKMPP NV12 retry");
    return -1;
  }
  LOG_WARN("RKMPP encoder rejected NV16, retrying MJPEG decode as NV12");
  auto worker = std::move(ctx->decode_worker);
  auto latest = worker ? worker->pause() : nullptr;
  free_pipeline(ctx);

  ctx->jpeg_output_nv12 = true;
  ctx->force_keyframe = true;
  if (init_decoder(ctx) != 0) {
    free_pipeline(ctx);
    if (worker)
      worker->stop();
    av_packet_free(&retry);
    return -1;
  }

  ret = encode_mjpeg_packet(ctx, retry, out_data, out_len, out_keyframe, out_pts_ms);
  av_packet_free(&retry);
  if (worker)
    worker->resume(ctx->dec_ctx, std::move(latest));
  ctx->decode_worker = std::move(worker);
  if (ret >= 0)
    return ret;
  free(*out_data);
  *out_data = nullptr;
  *out_len = 0;
  *out_keyframe = 0;
  return -1;
}

extern "C" int ffmpeg_hw_mjpeg_h26x_encode(FfmpegHwMjpegH26x* handle,
                                          const uint8_t* data, int len,
                                          int64_t pts_ms, uint8_t** out_data,
                                          int* out_len, int* out_keyframe,
                                          int64_t* out_pts_ms) {
  auto *ctx = reinterpret_cast<FfmpegHwMjpegH26xCtx*>(handle);
  if (!ctx || !ctx->dec_pkt || ctx->decode_worker || !data || len <= 0 || !out_data ||
      !out_len || !out_keyframe || !out_pts_ms) {
    set_last_error("Invalid parameters or unavailable synchronous MJPEG pipeline");
    return -1;
  }
  *out_data = nullptr;
  *out_len = 0;
  *out_keyframe = 0;
  *out_pts_ms = pts_ms;
  if (prepare_packet(ctx->dec_pkt, data, len, pts_ms) < 0)
    return -1;
  const int ret = encode_mjpeg_packet(ctx, ctx->dec_pkt, out_data, out_len,
                                     out_keyframe, out_pts_ms);
  return finish_mjpeg_encode(ctx, ret, ctx->dec_pkt, out_data, out_len,
                             out_keyframe, out_pts_ms);
}

extern "C" FfmpegHwMjpegInput *ffmpeg_hw_mjpeg_h26x_start_worker(
    FfmpegHwMjpegH26x *handle, FfmpegHwMjpegWakeup wakeup, void *user) {
  auto *ctx = reinterpret_cast<FfmpegHwMjpegH26xCtx *>(handle);
  if (!ctx || !ctx->dec_ctx || ctx->decode_worker || !wakeup)
    return nullptr;
  try {
    auto input = std::make_unique<FfmpegHwMjpegInput>();
    input->worker = std::make_shared<MjpegDecodeWorker>(ctx->dec_ctx, wakeup, user);
    ctx->decode_worker = input->worker;
    return input.release();
  } catch (const std::system_error &error) {
    set_last_error("Could not start RKMPP MJPEG decode worker: " + std::string(error.what()));
    return nullptr;
  }
}

extern "C" int ffmpeg_hw_mjpeg_h26x_submit(FfmpegHwMjpegInput *input,
                                         const uint8_t *data, int len, int64_t pts_ms) {
  if (!input || !data || len <= 0) {
    set_last_error("Invalid parameters or unavailable MJPEG decode worker");
    return -1;
  }
  auto job = std::make_unique<MjpegDecodeJob>();
  if (!job->packet || !job->frame) {
    set_last_error("Failed to allocate MJPEG decode job");
    return -1;
  }
  if (prepare_packet(job->packet, data, len, pts_ms) < 0)
    return -1;
  if (!input->worker->submit(std::move(job))) {
    set_last_error("MJPEG decode worker is stopped");
    return -1;
  }
  return 0;
}

extern "C" void ffmpeg_hw_mjpeg_h26x_free_input(FfmpegHwMjpegInput *input) {
  delete input;
}

extern "C" int ffmpeg_hw_mjpeg_h26x_ready(FfmpegHwMjpegH26x *handle) {
  auto *ctx = reinterpret_cast<FfmpegHwMjpegH26xCtx *>(handle);
  return ctx && ctx->decode_worker && ctx->decode_worker->has_output();
}

extern "C" void ffmpeg_hw_mjpeg_h26x_discard_ready(FfmpegHwMjpegH26x *handle) {
  auto *ctx = reinterpret_cast<FfmpegHwMjpegH26xCtx *>(handle);
  if (ctx && ctx->decode_worker)
    ctx->decode_worker->take_output();
}

extern "C" int ffmpeg_hw_mjpeg_h26x_encode_ready(FfmpegHwMjpegH26x *handle,
                                               uint8_t **out_data, int *out_len,
                                               int *out_keyframe, int64_t *out_pts_ms) {
  auto *ctx = reinterpret_cast<FfmpegHwMjpegH26xCtx *>(handle);
  if (!ctx || !ctx->decode_worker || !out_data || !out_len ||
      !out_keyframe || !out_pts_ms) {
    set_last_error("Invalid parameters or unavailable MJPEG encode stage");
    return -1;
  }
  *out_data = nullptr;
  *out_len = 0;
  *out_keyframe = 0;
  *out_pts_ms = 0;
  auto job = ctx->decode_worker->take_output();
  if (!job || job->result == AVERROR(EAGAIN) || job->result == AVERROR_EOF)
    return 0;
  if (job->result < 0) {
    set_last_error(make_err("RKMPP MJPEG decode failed", job->result));
    return -1;
  }
  const int ret = encode_decoded_frame(ctx, job->frame, job->packet->pts,
                                       out_data, out_len, out_keyframe, out_pts_ms);
  // The retry can close the decoder. Release this frame before it does.
  av_frame_unref(job->frame);
  return finish_mjpeg_encode(ctx, ret, job->packet, out_data, out_len,
                             out_keyframe, out_pts_ms);
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

  free_pipeline(ctx);

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
