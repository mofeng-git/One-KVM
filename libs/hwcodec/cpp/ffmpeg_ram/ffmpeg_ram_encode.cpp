// https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/encode_video.c

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavutil/imgutils.h>
#include <libavutil/log.h>
#include <libavutil/opt.h>
}

#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>

#include "common.h"

#define LOG_MODULE "FFMPEG_RAM_ENC"
#include <log.h>
#include <util.h>
#ifdef _WIN32
#include "win.h"
#endif

static thread_local std::string g_encoder_last_error;

static void set_encoder_last_error(const std::string &message) {
  g_encoder_last_error = message;
  LOG_ERROR(message);
}

static int calculate_offset_length(int pix_fmt, int height, const int *linesize,
                                   int *offset, int *length) {
  switch (pix_fmt) {
  case AV_PIX_FMT_YUV420P:
    offset[0] = linesize[0] * height;
    offset[1] = offset[0] + linesize[1] * height / 2;
    *length = offset[1] + linesize[2] * height / 2;
    break;
  case AV_PIX_FMT_NV12:
  case AV_PIX_FMT_NV21:
    offset[0] = linesize[0] * height;
    *length = offset[0] + linesize[1] * height / 2;
    break;
  case AV_PIX_FMT_NV16:
  case AV_PIX_FMT_NV24:
    offset[0] = linesize[0] * height;
    *length = offset[0] + linesize[1] * height;
    break;
  case AV_PIX_FMT_YUYV422:
  case AV_PIX_FMT_YVYU422:
  case AV_PIX_FMT_UYVY422:
    // Packed YUV 4:2:2 formats: single plane, 2 bytes per pixel
    // linesize[0] = width * 2 (YUYV/YVYU/UYVY are interleaved)
    offset[0] = 0;  // Only one plane
    *length = linesize[0] * height;
    break;
  case AV_PIX_FMT_RGB24:
  case AV_PIX_FMT_BGR24:
    offset[0] = 0;  // Only one plane
    *length = linesize[0] * height;
    break;
  default:
    LOG_ERROR(std::string("unsupported pixfmt") + std::to_string(pix_fmt));
    return -1;
  }

  return 0;
}

extern "C" int ffmpeg_ram_get_linesize_offset_length(int pix_fmt, int width,
                                                     int height, int align,
                                                     int *linesize, int *offset,
                                                     int *length) {
  AVFrame *frame = NULL;
  int ioffset[AV_NUM_DATA_POINTERS] = {0};
  int ilength = 0;
  int ret = -1;

  if (!(frame = av_frame_alloc())) {
    LOG_ERROR(std::string("Alloc frame failed"));
    goto _exit;
  }

  frame->format = pix_fmt;
  frame->width = width;
  frame->height = height;

  if ((ret = av_frame_get_buffer(frame, align)) < 0) {
    LOG_ERROR(std::string("av_frame_get_buffer, ret = ") + av_err2str(ret));
    goto _exit;
  }
  if (linesize) {
    for (int i = 0; i < AV_NUM_DATA_POINTERS; i++)
      linesize[i] = frame->linesize[i];
  }
  if (offset || length) {
    ret = calculate_offset_length(pix_fmt, height, frame->linesize, ioffset,
                                  &ilength);
    if (ret < 0)
      goto _exit;
  }
  if (offset) {
    for (int i = 0; i < AV_NUM_DATA_POINTERS; i++) {
      if (ioffset[i] == 0)
        break;
      offset[i] = ioffset[i];
    }
  }
  if (length)
    *length = ilength;

  ret = 0;
_exit:
  if (frame)
    av_frame_free(&frame);
  return ret;
}

namespace {
typedef void (*RamEncodeCallback)(const uint8_t *data, int len, int64_t pts,
                                  int key, const void *obj);
typedef void (*RamEncodePacketCallback)(void *packet, const uint8_t *data,
                                        int len, int64_t pts, int key,
                                        const void *obj);

class FFmpegRamEncoder {
public:
  AVCodecContext *c_ = NULL;
  AVFrame *frame_ = NULL;
  AVPacket *pkt_ = NULL;
  std::string name_;

  int width_ = 0;
  int height_ = 0;
  AVPixelFormat pixfmt_ = AV_PIX_FMT_NV12;
  int align_ = 0;
  int rc_ = 0;
  int quality_ = 0;
  int kbs_ = 0;
  int q_ = 0;
  int fps_ = 30;
  int gop_ = 0xFFFF;
  int thread_count_ = 1;
  int gpu_ = 0;
  RamEncodeCallback callback_ = NULL;
  RamEncodePacketCallback packet_callback_ = NULL;
  int offset_[AV_NUM_DATA_POINTERS] = {0};
  bool force_keyframe_ = false;  // Force next frame to be a keyframe

  AVHWDeviceType hw_device_type_ = AV_HWDEVICE_TYPE_NONE;
  AVPixelFormat hw_pixfmt_ = AV_PIX_FMT_NONE;
  AVBufferRef *hw_device_ctx_ = NULL;
  AVFrame *hw_frame_ = NULL;

  FFmpegRamEncoder(const char *name, int width, int height,
                   int pixfmt, int align, int fps, int gop, int rc, int quality,
                   int kbs, int q, int thread_count, int gpu,
                   RamEncodeCallback callback) {
    name_ = name;
    width_ = width;
    height_ = height;
    pixfmt_ = (AVPixelFormat)pixfmt;
    align_ = align;
    fps_ = fps;
    gop_ = gop;
    rc_ = rc;
    quality_ = quality;
    kbs_ = kbs;
    q_ = q;
    thread_count_ = thread_count;
    gpu_ = gpu;
    callback_ = callback;
    if (name_.find("vaapi") != std::string::npos) {
      hw_device_type_ = AV_HWDEVICE_TYPE_VAAPI;
      hw_pixfmt_ = AV_PIX_FMT_VAAPI;
    } else if (name_.find("nvenc") != std::string::npos) {
#ifdef _WIN32
      hw_device_type_ = AV_HWDEVICE_TYPE_D3D11VA;
      hw_pixfmt_ = AV_PIX_FMT_D3D11;
#endif
    }
  }

  ~FFmpegRamEncoder() {}

  void request_keyframe() {
    force_keyframe_ = true;
  }

  bool init(int *linesize, int *offset, int *length) {
    g_encoder_last_error.clear();
    const AVCodec *codec = NULL;

    int ret;

    if (!(codec = avcodec_find_encoder_by_name(name_.c_str()))) {
      set_encoder_last_error(std::string("Codec ") + name_ + " not found");
      return false;
    }

    if (!(c_ = avcodec_alloc_context3(codec))) {
      LOG_ERROR(std::string("Could not allocate video codec context"));
      return false;
    }

    if (hw_device_type_ != AV_HWDEVICE_TYPE_NONE) {
      std::string device = "";
#ifdef _WIN32
      if (name_.find("nvenc") != std::string::npos) {
        int index = Adapters::GetFirstAdapterIndex(
            AdapterVendor::ADAPTER_VENDOR_NVIDIA);
        if (index >= 0) {
          device = std::to_string(index);
        }
      }
#endif
      ret = av_hwdevice_ctx_create(&hw_device_ctx_, hw_device_type_,
                                   device.length() == 0 ? NULL : device.c_str(),
                                   NULL, 0);
      if (ret < 0) {
        LOG_ERROR(std::string("av_hwdevice_ctx_create failed"));
        return false;
      }
      if (set_hwframe_ctx() != 0) {
        LOG_ERROR(std::string("set_hwframe_ctx failed"));
        return false;
      }
      hw_frame_ = av_frame_alloc();
      if (!hw_frame_) {
        LOG_ERROR(std::string("av_frame_alloc failed"));
        return false;
      }
      if ((ret = av_hwframe_get_buffer(c_->hw_frames_ctx, hw_frame_, 0)) < 0) {
        LOG_ERROR(std::string("av_hwframe_get_buffer failed, ret = ") + av_err2str(ret));
        return false;
      }
      if (!hw_frame_->hw_frames_ctx) {
        LOG_ERROR(std::string("hw_frame_->hw_frames_ctx is NULL"));
        return false;
      }
    }

    if (!(frame_ = av_frame_alloc())) {
      LOG_ERROR(std::string("Could not allocate video frame"));
      return false;
    }
    frame_->format = pixfmt_;
    frame_->width = width_;
    frame_->height = height_;

    if ((ret = av_frame_get_buffer(frame_, align_)) < 0) {
      LOG_ERROR(std::string("av_frame_get_buffer failed, ret = ") + av_err2str(ret));
      return false;
    }

    if (!(pkt_ = av_packet_alloc())) {
      LOG_ERROR(std::string("Could not allocate video packet"));
      return false;
    }
    /* resolution must be a multiple of two */
    c_->width = width_;
    c_->height = height_;
    c_->pix_fmt =
        hw_pixfmt_ != AV_PIX_FMT_NONE ? hw_pixfmt_ : (AVPixelFormat)pixfmt_;
    c_->sw_pix_fmt = (AVPixelFormat)pixfmt_;
    util_encode::set_av_codec_ctx(c_, name_, kbs_, gop_, fps_, thread_count_);
    if (!util_encode::set_lantency_free(c_->priv_data, name_)) {
      LOG_ERROR(std::string("set_lantency_free failed, name: ") + name_);
      return false;
    }
    if (!util_encode::set_quality(c_->priv_data, name_, quality_)) {
      LOG_ERROR(std::string("set_quality failed, name: ") + name_);
      return false;
    }
    util_encode::set_rate_control(c_, name_, rc_, q_);
    util_encode::set_gpu(c_->priv_data, name_, gpu_);
    util_encode::force_hw(c_->priv_data, name_);
    util_encode::set_others(c_->priv_data, name_);
    if ((ret = avcodec_open2(c_, codec, NULL)) < 0) {
      set_encoder_last_error(std::string("avcodec_open2 failed, ret = ") +
                             av_err2str(ret) + ", name: " + name_);
      return false;
    }

    if (ffmpeg_ram_get_linesize_offset_length(pixfmt_, width_, height_, align_,
                                              NULL, offset_, length) != 0)
      return false;

    for (int i = 0; i < AV_NUM_DATA_POINTERS; i++) {
      linesize[i] = frame_->linesize[i];
      offset[i] = offset_[i];
    }
    return true;
  }

  int encode(const uint8_t *data, int length, const void *obj, uint64_t ms) {
    int ret;

    if ((ret = av_frame_make_writable(frame_)) != 0) {
      LOG_ERROR(std::string("av_frame_make_writable failed, ret = ") + av_err2str(ret));
      return ret;
    }
    if ((ret = fill_frame(frame_, data, length, offset_)) != 0)
      return ret;
    AVFrame *tmp_frame;
    if (hw_device_type_ != AV_HWDEVICE_TYPE_NONE) {
      if ((ret = av_hwframe_transfer_data(hw_frame_, frame_, 0)) < 0) {
        LOG_ERROR(std::string("av_hwframe_transfer_data failed, ret = ") + av_err2str(ret));
        return ret;
      }
      tmp_frame = hw_frame_;
    } else {
      tmp_frame = frame_;
    }

    return do_encode(tmp_frame, obj, ms);
  }

  int encode_packet(const uint8_t *data, int length, const void *obj,
                    uint64_t ms, RamEncodePacketCallback callback) {
    packet_callback_ = callback;
    int ret = encode(data, length, obj, ms);
    packet_callback_ = NULL;
    return ret;
  }

  void free_encoder() {
    if (pkt_)
      av_packet_free(&pkt_);
    if (frame_)
      av_frame_free(&frame_);
    if (hw_frame_)
      av_frame_free(&hw_frame_);
    if (hw_device_ctx_)
      av_buffer_unref(&hw_device_ctx_);
    if (c_)
      avcodec_free_context(&c_);
  }

  int set_bitrate(int kbs) {
    return util_encode::change_bit_rate(c_, name_, kbs) ? 0 : -1;
  }

private:
  int set_hwframe_ctx() {
    AVBufferRef *hw_frames_ref;
    AVHWFramesContext *frames_ctx = NULL;
    int err = 0;

    if (!(hw_frames_ref = av_hwframe_ctx_alloc(hw_device_ctx_))) {
      LOG_ERROR(std::string("av_hwframe_ctx_alloc failed"));
      return -1;
    }
    frames_ctx = (AVHWFramesContext *)(hw_frames_ref->data);
    frames_ctx->format = hw_pixfmt_;
    frames_ctx->sw_format = (AVPixelFormat)pixfmt_;
    frames_ctx->width = width_;
    frames_ctx->height = height_;
    frames_ctx->initial_pool_size = 1;
    if ((err = av_hwframe_ctx_init(hw_frames_ref)) < 0) {
      av_buffer_unref(&hw_frames_ref);
      return err;
    }
    c_->hw_frames_ctx = av_buffer_ref(hw_frames_ref);
    if (!c_->hw_frames_ctx) {
      LOG_ERROR(std::string("av_buffer_ref failed"));
      err = -1;
    }
    av_buffer_unref(&hw_frames_ref);
    return err;
  }

  int do_encode(AVFrame *frame, const void *obj, int64_t ms) {
    int ret;
    bool encoded = false;
    frame->pts = ms;

    // Force keyframe if requested
    if (force_keyframe_) {
      frame->pict_type = AV_PICTURE_TYPE_I;
      force_keyframe_ = false;
    } else {
      frame->pict_type = AV_PICTURE_TYPE_NONE;
    }

    ret = avcodec_send_frame(c_, frame);
    if (ret == AVERROR(EAGAIN)) {
      int drain_ret = receive_available_packets(obj, encoded);
      if (drain_ret < 0) {
        return drain_ret;
      }
      ret = avcodec_send_frame(c_, frame);
    }
    if (ret == AVERROR(EAGAIN)) {
      return encoded ? 0 : AVERROR(EAGAIN);
    }
    if (ret < 0) {
      LOG_ERROR(std::string("avcodec_send_frame failed, ret = ") + av_err2str(ret));
      return ret;
    }

    ret = receive_available_packets(obj, encoded);
    if (ret < 0) {
      return ret;
    }

    // If no packet is produced for this input frame, treat it as EAGAIN.
    // This is not a fatal error: encoders may buffer internally (e.g., startup delay).
    return encoded ? 0 : AVERROR(EAGAIN);
  }

  int receive_available_packets(const void *obj, bool &encoded) {
    int ret = 0;
    auto start = util::now();

    while (util::elapsed_ms(start) < DECODE_TIMEOUT_MS) {
      ret = avcodec_receive_packet(c_, pkt_);
      if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
        return 0;
      }
      if (ret < 0) {
        LOG_ERROR(std::string("avcodec_receive_packet failed, ret = ") + av_err2str(ret));
        av_packet_unref(pkt_);
        return ret;
      }
      if (!pkt_->data || !pkt_->size) {
        LOG_WARN(std::string("avcodec_receive_packet returned empty packet"));
        av_packet_unref(pkt_);
        continue;
      }
      encoded = true;
      if (packet_callback_) {
        AVPacket *owned_pkt = av_packet_clone(pkt_);
        if (!owned_pkt) {
          LOG_ERROR("av_packet_clone failed");
          av_packet_unref(pkt_);
          return AVERROR(ENOMEM);
        }
        packet_callback_(owned_pkt, owned_pkt->data, owned_pkt->size,
                         owned_pkt->pts, owned_pkt->flags & AV_PKT_FLAG_KEY,
                         obj);
      } else {
        callback_(pkt_->data, pkt_->size, pkt_->pts,
                  pkt_->flags & AV_PKT_FLAG_KEY, obj);
      }
      av_packet_unref(pkt_);
    }

    return 0;
  }

  int copy_plane(uint8_t *dst, int dst_stride, const uint8_t *src,
                 int src_stride, int row_bytes, int rows) {
    if (!dst || !src || dst_stride < row_bytes || src_stride < row_bytes) {
      return -1;
    }
    if (rows <= 0 || row_bytes <= 0) {
      return 0;
    }
    if (dst_stride == row_bytes && src_stride == row_bytes) {
      memcpy(dst, src, static_cast<size_t>(row_bytes) * rows);
      return 0;
    }
    for (int y = 0; y < rows; y++) {
      memcpy(dst + y * dst_stride, src + y * src_stride, row_bytes);
    }
    return 0;
  }

  int fill_frame(AVFrame *frame, const uint8_t *data, int data_length,
                 const int *const) {
    const int src_y_stride = width_;
    const int src_packed_stride = width_ * bytes_per_pixel(frame->format);
    const int src_uv_stride = width_;
    const int src_y_size = width_ * frame->height;
    const int src_420_chroma_size = (width_ / 2) * (frame->height / 2);
    switch (frame->format) {
    case AV_PIX_FMT_NV12:
    case AV_PIX_FMT_NV21:
      if (data_length <
          frame->height * src_y_stride + frame->height / 2 * src_uv_stride) {
        LOG_ERROR(std::string("fill_frame: NV12/NV21 data length error. data_length:") +
                  std::to_string(data_length) +
                  ", width:" + std::to_string(width_) +
                  ", height:" + std::to_string(frame->height));
        return -1;
      }
      if (copy_plane(frame->data[0], frame->linesize[0], data, src_y_stride,
                     width_, frame->height) != 0 ||
          copy_plane(frame->data[1], frame->linesize[1], data + src_y_size,
                     src_uv_stride, width_, frame->height / 2) != 0) {
        LOG_ERROR("fill_frame: NV12/NV21 copy failed");
        return -1;
      }
      break;
    case AV_PIX_FMT_NV16:
      if (data_length <
          frame->height * src_y_stride + frame->height * src_uv_stride) {
        LOG_ERROR(std::string("fill_frame: NV16 data length error. data_length:") +
                  std::to_string(data_length) +
                  ", width:" + std::to_string(width_) +
                  ", height:" + std::to_string(frame->height));
        return -1;
      }
      if (copy_plane(frame->data[0], frame->linesize[0], data, src_y_stride,
                     width_, frame->height) != 0 ||
          copy_plane(frame->data[1], frame->linesize[1], data + src_y_size,
                     src_uv_stride, width_, frame->height) != 0) {
        LOG_ERROR("fill_frame: NV16 copy failed");
        return -1;
      }
      break;
    case AV_PIX_FMT_NV24: {
      const int src_nv24_uv_stride = width_ * 2;
      if (data_length <
          frame->height * src_y_stride + frame->height * src_nv24_uv_stride) {
        LOG_ERROR(std::string("fill_frame: NV24 data length error. data_length:") +
                  std::to_string(data_length) +
                  ", width:" + std::to_string(width_) +
                  ", height:" + std::to_string(frame->height));
        return -1;
      }
      if (copy_plane(frame->data[0], frame->linesize[0], data, src_y_stride,
                     width_, frame->height) != 0 ||
          copy_plane(frame->data[1], frame->linesize[1], data + src_y_size,
                     src_nv24_uv_stride, width_ * 2, frame->height) != 0) {
        LOG_ERROR("fill_frame: NV24 copy failed");
        return -1;
      }
      break;
    }
    case AV_PIX_FMT_YUV420P:
      if (data_length <
          width_ * frame->height + (width_ / 2) * (frame->height / 2) * 2) {
        LOG_ERROR(std::string("fill_frame: 420P data length error. data_length:") +
                  std::to_string(data_length) +
                  ", width:" + std::to_string(width_) +
                  ", height:" + std::to_string(frame->height));
        return -1;
      }
      if (copy_plane(frame->data[0], frame->linesize[0], data, width_,
                     width_, frame->height) != 0 ||
          copy_plane(frame->data[1], frame->linesize[1], data + src_y_size,
                     width_ / 2, width_ / 2, frame->height / 2) != 0 ||
          copy_plane(frame->data[2], frame->linesize[2],
                     data + src_y_size + src_420_chroma_size,
                     width_ / 2, width_ / 2, frame->height / 2) != 0) {
        LOG_ERROR("fill_frame: 420P copy failed");
        return -1;
      }
      break;
    case AV_PIX_FMT_YUYV422:
    case AV_PIX_FMT_YVYU422:
    case AV_PIX_FMT_UYVY422:
      // Packed YUV 4:2:2 formats: single plane, linesize[0] = width * 2
      if (data_length < frame->height * src_packed_stride) {
        LOG_ERROR(std::string("fill_frame: YUYV422 data length error. data_length:") +
                  std::to_string(data_length) +
                  ", stride:" + std::to_string(src_packed_stride) +
                  ", height:" + std::to_string(frame->height));
        return -1;
      }
      if (copy_plane(frame->data[0], frame->linesize[0], data,
                     src_packed_stride, src_packed_stride, frame->height) != 0) {
        LOG_ERROR("fill_frame: YUYV422 copy failed");
        return -1;
      }
      break;
    case AV_PIX_FMT_RGB24:
    case AV_PIX_FMT_BGR24:
      if (data_length < frame->height * src_packed_stride) {
        LOG_ERROR(std::string("fill_frame: RGB24/BGR24 data length error. data_length:") +
                  std::to_string(data_length) +
                  ", stride:" + std::to_string(src_packed_stride) +
                  ", height:" + std::to_string(frame->height));
        return -1;
      }
      if (copy_plane(frame->data[0], frame->linesize[0], data,
                     src_packed_stride, src_packed_stride, frame->height) != 0) {
        LOG_ERROR("fill_frame: RGB24/BGR24 copy failed");
        return -1;
      }
      break;
    default:
      LOG_ERROR(std::string("fill_frame: unsupported format, ") +
                std::to_string(frame->format));
      return -1;
    }
    return 0;
  }

  int bytes_per_pixel(int pix_fmt) {
    switch (pix_fmt) {
    case AV_PIX_FMT_YUYV422:
    case AV_PIX_FMT_YVYU422:
    case AV_PIX_FMT_UYVY422:
      return 2;
    case AV_PIX_FMT_RGB24:
    case AV_PIX_FMT_BGR24:
      return 3;
    default:
      return 1;
    }
  }
};

} // namespace

extern "C" FFmpegRamEncoder *
ffmpeg_ram_new_encoder(const char *name, int width,
                       int height, int pixfmt, int align, int fps, int gop,
                       int rc, int quality, int kbs, int q, int thread_count,
                       int gpu, int *linesize, int *offset, int *length,
                       RamEncodeCallback callback) {
  FFmpegRamEncoder *encoder = NULL;
  try {
    encoder = new FFmpegRamEncoder(name, width, height, pixfmt, align,
                                   fps, gop, rc, quality, kbs, q, thread_count,
                                   gpu, callback);
    if (encoder) {
      if (encoder->init(linesize, offset, length)) {
        return encoder;
      }
    }
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("new FFmpegRamEncoder failed, ") + std::string(e.what()));
  }
  if (encoder) {
    encoder->free_encoder();
    delete encoder;
    encoder = NULL;
  }
  return NULL;
}

extern "C" int ffmpeg_ram_encode(FFmpegRamEncoder *encoder, const uint8_t *data,
                                 int length, const void *obj, uint64_t ms) {
  try {
    return encoder->encode(data, length, obj, ms);
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("ffmpeg_ram_encode failed, ") + std::string(e.what()));
  }
  return -1;
}

extern "C" void ffmpeg_ram_free_encoder(FFmpegRamEncoder *encoder) {
  try {
    if (!encoder)
      return;
    encoder->free_encoder();
    delete encoder;
    encoder = NULL;
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("free encoder failed, ") + std::string(e.what()));
  }
}

extern "C" int ffmpeg_ram_encode_packet(FFmpegRamEncoder *encoder,
                                        const uint8_t *data, int length,
                                        const void *obj, uint64_t ms,
                                        RamEncodePacketCallback callback) {
  try {
    return encoder->encode_packet(data, length, obj, ms, callback);
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("encode_packet failed, ") + std::string(e.what()));
    return -1;
  }
}

extern "C" void ffmpeg_ram_free_packet(void *packet) {
  AVPacket *pkt = reinterpret_cast<AVPacket *>(packet);
  if (pkt) {
    av_packet_free(&pkt);
  }
}

extern "C" int ffmpeg_ram_set_bitrate(FFmpegRamEncoder *encoder, int kbs) {
  try {
    return encoder->set_bitrate(kbs);
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("ffmpeg_ram_set_bitrate failed, ") + std::string(e.what()));
  }
  return -1;
}

extern "C" void ffmpeg_ram_request_keyframe(FFmpegRamEncoder *encoder) {
  try {
    if (encoder) {
      encoder->request_keyframe();
    }
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("ffmpeg_ram_request_keyframe failed, ") + std::string(e.what()));
  }
}

extern "C" const char *ffmpeg_ram_encoder_last_error(void) {
  return g_encoder_last_error.c_str();
}
