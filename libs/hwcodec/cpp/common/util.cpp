extern "C" {
#include <libavutil/opt.h>
}

#include "util.h"
#include <limits>
#include <map>
#include <string.h>
#include <vector>

#include "common.h"

#define LOG_MODULE "UTIL"
#include "log.h"

#ifndef FF_PROFILE_H264_BASELINE
#define FF_PROFILE_H264_BASELINE 66
#endif
#ifndef FF_PROFILE_H264_HIGH
#define FF_PROFILE_H264_HIGH 100
#endif
#ifndef FF_PROFILE_HEVC_MAIN
#define FF_PROFILE_HEVC_MAIN 1
#endif

namespace {

// Helper function: check if encoder is software H264 (libx264)
bool is_software_h264(const std::string &name) {
  if (name != "h264" && name != "libx264") return false;
  // Exclude all hardware encoders
  static const char* hw_suffixes[] = {
    "nvenc", "amf", "qsv", "vaapi", "rkmpp",
    "v4l2m2m", "videotoolbox", "mediacodec", "_mf"
  };
  for (const auto& suffix : hw_suffixes) {
    if (name.find(suffix) != std::string::npos) return false;
  }
  return true;
}

// Helper function: check if encoder is software HEVC (libx265)
bool is_software_hevc(const std::string &name) {
  if (name != "hevc" && name != "libx265") return false;
  static const char* hw_suffixes[] = {
    "nvenc", "amf", "qsv", "vaapi", "rkmpp",
    "v4l2m2m", "videotoolbox", "mediacodec", "_mf"
  };
  for (const auto& suffix : hw_suffixes) {
    if (name.find(suffix) != std::string::npos) return false;
  }
  return true;
}

} // anonymous namespace

namespace util_encode {

void set_av_codec_ctx(AVCodecContext *c, const std::string &name, int kbs,
                      int gop, int fps, int thread_count) {
  c->has_b_frames = 0;
  c->max_b_frames = 0;
  if (gop > 0 && gop < std::numeric_limits<int16_t>::max()) {
    c->gop_size = gop;
    c->keyint_min = gop; // Match keyint_min to gop for consistent keyframe interval
  } else if (name.find("vaapi") != std::string::npos) {
    c->gop_size = fps > 0 ? fps : 30; // Default to 1 second keyframe interval
    c->keyint_min = c->gop_size;
  } else if (name.find("qsv") != std::string::npos) {
    c->gop_size = fps > 0 ? fps : 30;
    c->keyint_min = c->gop_size;
  } else {
    c->gop_size = fps > 0 ? fps : 30;
    c->keyint_min = c->gop_size;
  }
  /* put sample parameters */
  // https://github.com/FFmpeg/FFmpeg/blob/415f012359364a77e8394436f222b74a8641a3ee/libavcodec/encode.c#L581
  if (kbs > 0) {
    c->bit_rate = kbs * 1000;
    if (name.find("qsv") != std::string::npos) {
      c->rc_max_rate = c->bit_rate;
      c->bit_rate--; // cbr with vbr
    }
  }
  /* frames per second */
  c->time_base = av_make_q(1, 1000);
  c->framerate = av_make_q(fps, 1);
  c->flags |= AV_CODEC_FLAG2_LOCAL_HEADER;
  c->flags |= AV_CODEC_FLAG_LOW_DELAY;

  // Threading configuration: use frame-based threading for software encoders
  if (is_software_h264(name) || is_software_hevc(name)) {
    // Software encoders benefit from frame-level parallelism
    c->thread_type = FF_THREAD_FRAME;
    c->thread_count = thread_count > 0 ? thread_count : 4;
    c->slices = 1;
  } else {
    // Hardware encoders typically use slice-based threading
    c->slices = 1;
    c->thread_type = FF_THREAD_SLICE;
    c->thread_count = c->slices;
  }

  // https://github.com/obsproject/obs-studio/blob/3cc7dc0e7cf8b01081dc23e432115f7efd0c8877/plugins/obs-ffmpeg/obs-ffmpeg-mux.c#L160
  c->color_range = AVCOL_RANGE_MPEG;
  c->colorspace = AVCOL_SPC_SMPTE170M;
  c->color_primaries = AVCOL_PRI_SMPTE170M;
  c->color_trc = AVCOL_TRC_SMPTE170M;

  // Profile selection: use BASELINE for software H264 (faster, simpler)
  if (is_software_h264(name)) {
    c->profile = FF_PROFILE_H264_BASELINE;  // Simpler profile for real-time
  } else if (name.find("h264") != std::string::npos) {
    c->profile = FF_PROFILE_H264_HIGH;
  } else if (name.find("hevc") != std::string::npos) {
    c->profile = FF_PROFILE_HEVC_MAIN;
  }
}

bool set_lantency_free(void *priv_data, const std::string &name) {
  int ret;

  if (name.find("nvenc") != std::string::npos) {
    if ((ret = av_opt_set(priv_data, "delay", "0", 0)) < 0) {
      LOG_ERROR(std::string("nvenc set_lantency_free failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  if (name.find("amf") != std::string::npos) {
    if ((ret = av_opt_set(priv_data, "query_timeout", "1000", 0)) < 0) {
      LOG_ERROR(std::string("amf set_lantency_free failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  if (name.find("qsv") != std::string::npos) {
    if ((ret = av_opt_set(priv_data, "async_depth", "1", 0)) < 0) {
      LOG_ERROR(std::string("qsv set_lantency_free failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  if (name.find("vaapi") != std::string::npos) {
    if ((ret = av_opt_set(priv_data, "async_depth", "1", 0)) < 0) {
      LOG_ERROR(std::string("vaapi set_lantency_free failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  // RKMPP (Rockchip MPP) hardware encoder - minimize buffer latency
  if (name.find("rkmpp") != std::string::npos) {
    // Set async_depth to 1 for minimal buffering (0 = synchronous, higher = more buffering)
    if ((ret = av_opt_set(priv_data, "async_depth", "1", 0)) < 0) {
      LOG_WARN(std::string("rkmpp set async_depth failed, ret = ") + av_err2str(ret));
      // Not fatal - older FFmpeg versions may not support this option
    }
  }
  // V4L2 M2M hardware encoder - minimize buffer latency
  if (name.find("v4l2m2m") != std::string::npos) {
    // Minimize number of output buffers for lower latency
    if ((ret = av_opt_set_int(priv_data, "num_output_buffers", 4, 0)) < 0) {
      LOG_WARN(std::string("v4l2m2m set num_output_buffers failed, ret = ") + av_err2str(ret));
      // Not fatal
    }
    if ((ret = av_opt_set_int(priv_data, "num_capture_buffers", 4, 0)) < 0) {
      LOG_WARN(std::string("v4l2m2m set num_capture_buffers failed, ret = ") + av_err2str(ret));
      // Not fatal
    }
  }
  if (name.find("videotoolbox") != std::string::npos) {
    if ((ret = av_opt_set_int(priv_data, "realtime", 1, 0)) < 0) {
      LOG_ERROR(std::string("videotoolbox set realtime failed, ret = ") + av_err2str(ret));
      return false;
    }
    if ((ret = av_opt_set_int(priv_data, "prio_speed", 1, 0)) < 0) {
      LOG_ERROR(std::string("videotoolbox set prio_speed failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  // libvpx (VP8/VP9) - set realtime mode to avoid frame lag
  if (name.find("libvpx") != std::string::npos) {
    // deadline: realtime for low-latency streaming
    if ((ret = av_opt_set(priv_data, "deadline", "realtime", 0)) < 0) {
      LOG_ERROR(std::string("libvpx set deadline realtime failed, ret = ") + av_err2str(ret));
      return false;
    }
    // cpu-used: 6 is good balance for real-time (0-8, higher = faster but lower quality)
    if ((ret = av_opt_set_int(priv_data, "cpu-used", 6, 0)) < 0) {
      LOG_ERROR(std::string("libvpx set cpu-used failed, ret = ") + av_err2str(ret));
      return false;
    }
    // lag-in-frames: 0 disables frame lag (important for real-time)
    if ((ret = av_opt_set_int(priv_data, "lag-in-frames", 0, 0)) < 0) {
      LOG_ERROR(std::string("libvpx set lag-in-frames failed, ret = ") + av_err2str(ret));
      return false;
    }
    // row-mt: enable row-based multithreading for VP9
    if (name.find("vp9") != std::string::npos) {
      if ((ret = av_opt_set_int(priv_data, "row-mt", 1, 0)) < 0) {
        LOG_ERROR(std::string("libvpx-vp9 set row-mt failed, ret = ") + av_err2str(ret));
        // row-mt failure is not fatal
      }
    }
  }
  // libx264 software encoder - zero latency tuning
  if (is_software_h264(name)) {
    // zerolatency: disable B-frames, reduce lookahead, disable CABAC etc.
    if ((ret = av_opt_set(priv_data, "tune", "zerolatency", 0)) < 0) {
      LOG_ERROR(std::string("libx264 set tune zerolatency failed, ret = ") + av_err2str(ret));
      // tune failure is not fatal, continue
    }
    // Disable B-frame adaptation (extra safety, zerolatency already includes this)
    av_opt_set_int(priv_data, "b-adapt", 0, 0);
    // Set low rc-lookahead for minimal latency
    av_opt_set_int(priv_data, "rc-lookahead", 0, 0);
    // Use sliced-threads for lower latency (encode slices in parallel within same frame)
    av_opt_set_int(priv_data, "sliced-threads", 1, 0);
    // Disable mb-tree for lower memory and faster encoding
    av_opt_set_int(priv_data, "mbtree", 0, 0);
    // Disable adaptive quantization for speed
    av_opt_set_int(priv_data, "aq-mode", 0, 0);
    // Use simpler motion estimation for speed
    av_opt_set(priv_data, "me", "dia", 0);  // diamond search (fastest)
    // Reduce subpixel motion estimation refinement
    av_opt_set_int(priv_data, "subq", 1, 0);  // 1 = fastest
    // Reduce reference frames for speed
    av_opt_set_int(priv_data, "refs", 1, 0);
  }
  // libx265 software encoder - zero latency tuning
  if (is_software_hevc(name)) {
    if ((ret = av_opt_set(priv_data, "tune", "zerolatency", 0)) < 0) {
      LOG_ERROR(std::string("libx265 set tune zerolatency failed, ret = ") + av_err2str(ret));
      // tune failure is not fatal, continue
    }
    // x265 specific low-latency parameters
    // bframes=0: no B-frames
    // rc-lookahead=0: no lookahead
    // no-slices-wpp=1: disable wavefront parallel processing for lower latency
    // frame-threads=1: single frame thread for lower latency
    av_opt_set(priv_data, "x265-params",
               "bframes=0:rc-lookahead=0:ref=1:no-b-adapt=1:aq-mode=0", 0);
  }
  return true;
}

bool set_quality(void *priv_data, const std::string &name, int quality) {
  int ret = -1;

  if (name.find("nvenc") != std::string::npos) {
    switch (quality) {
    // p7 isn't zero lantency
    case Quality_Medium:
      if ((ret = av_opt_set(priv_data, "preset", "p4", 0)) < 0) {
        LOG_ERROR(std::string("nvenc set opt preset p4 failed, ret = ") + av_err2str(ret));
        return false;
      }
      break;
    case Quality_Low:
      if ((ret = av_opt_set(priv_data, "preset", "p1", 0)) < 0) {
        LOG_ERROR(std::string("nvenc set opt preset p1 failed, ret = ") + av_err2str(ret));
        return false;
      }
      break;
    default:
      break;
    }
  }
  if (name.find("amf") != std::string::npos) {
    switch (quality) {
    case Quality_High:
      if ((ret = av_opt_set(priv_data, "quality", "quality", 0)) < 0) {
        LOG_ERROR(std::string("amf set opt quality quality failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
      break;
    case Quality_Medium:
      if ((ret = av_opt_set(priv_data, "quality", "balanced", 0)) < 0) {
        LOG_ERROR(std::string("amf set opt quality balanced failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
      break;
    case Quality_Low:
      if ((ret = av_opt_set(priv_data, "quality", "speed", 0)) < 0) {
        LOG_ERROR(std::string("amf set opt quality speed failed, ret = ") + av_err2str(ret));
        return false;
      }
      break;
    default:
      break;
    }
  }
  if (name.find("qsv") != std::string::npos) {
    switch (quality) {
    case Quality_High:
      if ((ret = av_opt_set(priv_data, "preset", "veryslow", 0)) < 0) {
        LOG_ERROR(std::string("qsv set opt preset veryslow failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
      break;
    case Quality_Medium:
      if ((ret = av_opt_set(priv_data, "preset", "medium", 0)) < 0) {
        LOG_ERROR(std::string("qsv set opt preset medium failed, ret = ") + av_err2str(ret));
        return false;
      }
      break;
    case Quality_Low:
      if ((ret = av_opt_set(priv_data, "preset", "veryfast", 0)) < 0) {
        LOG_ERROR(std::string("qsv set opt preset veryfast failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
      break;
    default:
      break;
    }
  }
  if (name.find("mediacodec") != std::string::npos) {
    if (name.find("h264") != std::string::npos) {
      if ((ret = av_opt_set(priv_data, "level", "5.1", 0)) < 0) {
        LOG_ERROR(std::string("mediacodec set opt level 5.1 failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
    }
    if (name.find("hevc") != std::string::npos) {
      // https:en.wikipedia.org/wiki/High_Efficiency_Video_Coding_tiers_and_levels
      if ((ret = av_opt_set(priv_data, "level", "h5.1", 0)) < 0) {
        LOG_ERROR(std::string("mediacodec set opt level h5.1 failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
    }
  }
  // libx264 software encoder presets
  if (is_software_h264(name)) {
    const char* preset = nullptr;
    switch (quality) {
    case Quality_High:
      preset = "veryfast";  // Good quality while maintaining fast encoding
      break;
    case Quality_Medium:
      preset = "superfast"; // Balance between speed and quality
      break;
    case Quality_Low:
      preset = "ultrafast"; // Fastest speed, lowest CPU usage
      break;
    default:
      preset = "superfast"; // Default to superfast for embedded devices
      break;
    }
    if ((ret = av_opt_set(priv_data, "preset", preset, 0)) < 0) {
      LOG_ERROR(std::string("libx264 set preset ") + preset + " failed, ret = " + av_err2str(ret));
      return false;
    }
  }
  // libx265 software encoder presets
  if (is_software_hevc(name)) {
    const char* preset = nullptr;
    switch (quality) {
    case Quality_High:
      preset = "veryfast";
      break;
    case Quality_Medium:
      preset = "superfast";
      break;
    case Quality_Low:
      preset = "ultrafast";
      break;
    default:
      preset = "superfast";
      break;
    }
    if ((ret = av_opt_set(priv_data, "preset", preset, 0)) < 0) {
      LOG_ERROR(std::string("libx265 set preset ") + preset + " failed, ret = " + av_err2str(ret));
      return false;
    }
  }
  return true;
}

struct CodecOptions {
  std::string codec_name;
  std::string option_name;
  std::map<int, std::string> rc_values;
};

bool set_rate_control(AVCodecContext *c, const std::string &name, int rc,
                      int q) {
  if (name.find("qsv") != std::string::npos) {
    // https://github.com/LizardByte/Sunshine/blob/3e47cd3cc8fd37a7a88be82444ff4f3c0022856b/src/video.cpp#L1635
    c->strict_std_compliance = FF_COMPLIANCE_UNOFFICIAL;
  }
  std::vector<CodecOptions> codecs = {
      {"nvenc", "rc", {{RC_CBR, "cbr"}, {RC_VBR, "vbr"}}},
      {"amf", "rc", {{RC_CBR, "cbr"}, {RC_VBR, "vbr_latency"}}},
      {"mediacodec",
       "bitrate_mode",
       {{RC_CBR, "cbr"}, {RC_VBR, "vbr"}, {RC_CQ, "cq"}}},
      // {"videotoolbox", "constant_bit_rate", {{RC_CBR, "1"}}},
    };

  for (const auto &codec : codecs) {
    if (name.find(codec.codec_name) != std::string::npos) {
      auto it = codec.rc_values.find(rc);
      if (it != codec.rc_values.end()) {
        int ret = av_opt_set(c->priv_data, codec.option_name.c_str(),
                             it->second.c_str(), 0);
        if (ret < 0) {
          LOG_ERROR(codec.codec_name + " set opt " + codec.option_name + " " +
                    it->second + " failed, ret = " + av_err2str(ret));
          return false;
        }
        if (name.find("mediacodec") != std::string::npos) {
          if (rc == RC_CQ) {
            if (q >= 0 && q <= 51) {
              c->global_quality = q;
            }
          }
        }
      }
      break;
    }
  }

  return true;
}
bool set_gpu(void *priv_data, const std::string &name, int gpu) {
  int ret;
  if (gpu < 0)
    return -1;
  if (name.find("nvenc") != std::string::npos) {
    if ((ret = av_opt_set_int(priv_data, "gpu", gpu, 0)) < 0) {
      LOG_ERROR(std::string("nvenc set gpu failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  return true;
}

bool force_hw(void *priv_data, const std::string &name) {
  int ret;
  if (name.find("_mf") != std::string::npos) {
    if ((ret = av_opt_set_int(priv_data, "hw_encoding", 1, 0)) < 0) {
      LOG_ERROR(std::string("mediafoundation set hw_encoding failed, ret = ") +
                av_err2str(ret));
      return false;
    }
  }
  if (name.find("videotoolbox") != std::string::npos) {
    if ((ret = av_opt_set_int(priv_data, "allow_sw", 0, 0)) < 0) {
      LOG_ERROR(std::string("mediafoundation set allow_sw failed, ret = ") +
                av_err2str(ret));
      return false;
    }
  }
  return true;
}

bool set_others(void *priv_data, const std::string &name) {
  int ret;
  if (name.find("_mf") != std::string::npos) {
    // ff_eAVScenarioInfo_DisplayRemoting = 1
    if ((ret = av_opt_set_int(priv_data, "scenario", 1, 0)) < 0) {
      LOG_ERROR(std::string("mediafoundation set scenario failed, ret = ") +
                av_err2str(ret));
      return false;
    }
  }
  // NOTE: Removed idr_interval = INT_MAX for VAAPI.
  // This was disabling automatic keyframe generation.
  // The encoder should respect c->gop_size for keyframe interval.
  return true;
}

bool change_bit_rate(AVCodecContext *c, const std::string &name, int kbs) {
  if (kbs > 0) {
    c->bit_rate = kbs * 1000;
    if (name.find("qsv") != std::string::npos) {
      c->rc_max_rate = c->bit_rate;
    }
  }
  return true;
}

void vram_encode_test_callback(const uint8_t *data, int32_t len, int32_t key, const void *obj, int64_t pts) {
  (void)data;
  (void)len;
  (void)pts;
  if (obj) {
    int32_t *pkey = (int32_t *)obj;
    *pkey = key;
  }
}

} // namespace util_encode

namespace util_decode {

static bool g_flag_could_not_find_ref_with_poc = false;

bool has_flag_could_not_find_ref_with_poc() {
  bool v = g_flag_could_not_find_ref_with_poc;
  g_flag_could_not_find_ref_with_poc = false;
  return v;
}

} // namespace util_decode

extern "C" void hwcodec_set_flag_could_not_find_ref_with_poc() {
  util_decode::g_flag_could_not_find_ref_with_poc = true;
}
