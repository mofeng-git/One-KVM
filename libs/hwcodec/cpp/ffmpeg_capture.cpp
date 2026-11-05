#define NOMINMAX
#include "ffmpeg_capture_ffi.h"

#include <Windows.h>
#include <dshow.h>
#include <dvdmedia.h>
extern "C" {
#include <libavcodec/codec_id.h>
#include <libavdevice/avdevice.h>
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>
#include <libavutil/error.h>
#include <libavutil/pixfmt.h>
}
#include <atomic>
#include <algorithm>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>


#pragma comment(lib, "strmiids")

thread_local std::string g_last_error;

struct HwcodecDshowCaptureContext {
  AVFormatContext* format_ctx = nullptr;
  int stream_index = -1;
  int width = 0;
  int height = 0;
  int pixel_format = HWCODEC_CAPTURE_FMT_UNKNOWN;
  int stride = 0;
  int timeout_ms = 2000;
  std::atomic<long long> deadline_ms{0};
  std::atomic<int> timed_out{0};
  uint64_t sequence = 0;
};

namespace {
struct DshowCapabilityEntry {
  std::string format;
  int width = 0;
  int height = 0;
  std::vector<int> fps;
};

const char* requested_pixel_format_name(int requested_format);

void set_last_error(const std::string& message) {
  g_last_error = message;
}

std::string ffmpeg_error(int errnum) {
  char buffer[AV_ERROR_MAX_STRING_SIZE] = {0};
  av_make_error_string(buffer, sizeof(buffer), errnum);
  return std::string(buffer);
}

long long now_ms() {
  return static_cast<long long>(GetTickCount64());
}

std::string wide_to_utf8(const wchar_t* value) {
  if (!value) {
    return std::string();
  }
  int size = WideCharToMultiByte(CP_UTF8, 0, value, -1, nullptr, 0, nullptr, nullptr);
  if (size <= 1) {
    return std::string();
  }
  std::string result(static_cast<size_t>(size - 1), '\0');
  WideCharToMultiByte(
      CP_UTF8,
      0,
      value,
      -1,
      result.empty() ? nullptr : &result[0],
      size,
      nullptr,
      nullptr);
  return result;
}

void add_fps_candidate(std::vector<int>* fps, LONGLONG interval_100ns) {
  if (!fps || interval_100ns <= 0) {
    return;
  }

  double fps_value = 10000000.0 / static_cast<double>(interval_100ns);
  int rounded = static_cast<int>(fps_value + 0.5);
  if (rounded <= 0) {
    return;
  }
  if (std::find(fps->begin(), fps->end(), rounded) == fps->end()) {
    fps->push_back(rounded);
  }
}

void normalize_fps(std::vector<int>* fps) {
  if (!fps) {
    return;
  }
  std::sort(fps->begin(), fps->end(), std::greater<int>());
  fps->erase(std::unique(fps->begin(), fps->end()), fps->end());
}

const char* media_subtype_to_format(const GUID& subtype) {
  if (subtype == MEDIASUBTYPE_MJPG) {
    return "MJPEG";
  }
  if (subtype == MEDIASUBTYPE_YUY2) {
    return "YUYV";
  }
  if (subtype == MEDIASUBTYPE_UYVY) {
    return "UYVY";
  }
  if (subtype == MEDIASUBTYPE_YVYU) {
    return "YVYU";
  }
  if (subtype == MEDIASUBTYPE_NV12) {
    return "NV12";
  }
  if (subtype == MEDIASUBTYPE_RGB24) {
    return "RGB24";
  }
  if (subtype == MEDIASUBTYPE_RGB32) {
    return "BGR24";
  }
  if (subtype == MEDIASUBTYPE_IYUV) {
    return "YUV420";
  }
  if (subtype == MEDIASUBTYPE_YV12) {
    return "YVU420";
  }
  return nullptr;
}

void free_media_type(AM_MEDIA_TYPE* media_type) {
  if (!media_type) {
    return;
  }
  if (media_type->cbFormat != 0) {
    CoTaskMemFree(media_type->pbFormat);
    media_type->cbFormat = 0;
    media_type->pbFormat = nullptr;
  }
  if (media_type->pUnk != nullptr) {
    media_type->pUnk->Release();
    media_type->pUnk = nullptr;
  }
  CoTaskMemFree(media_type);
}

bool fill_capability_entry(
    const AM_MEDIA_TYPE* media_type,
    const VIDEO_STREAM_CONFIG_CAPS* caps,
    DshowCapabilityEntry* out_entry) {
  if (!media_type || !out_entry) {
    return false;
  }

  const char* format = media_subtype_to_format(media_type->subtype);
  if (!format) {
    return false;
  }

  LONG width = 0;
  LONG height = 0;
  REFERENCE_TIME avg_time_per_frame = 0;

  if (media_type->formattype == FORMAT_VideoInfo && media_type->pbFormat &&
      media_type->cbFormat >= sizeof(VIDEOINFOHEADER)) {
    const auto* info = reinterpret_cast<const VIDEOINFOHEADER*>(media_type->pbFormat);
    width = info->bmiHeader.biWidth;
    height = std::abs(info->bmiHeader.biHeight);
    avg_time_per_frame = info->AvgTimePerFrame;
  } else if (media_type->formattype == FORMAT_VideoInfo2 && media_type->pbFormat &&
             media_type->cbFormat >= sizeof(VIDEOINFOHEADER2)) {
    const auto* info = reinterpret_cast<const VIDEOINFOHEADER2*>(media_type->pbFormat);
    width = info->bmiHeader.biWidth;
    height = std::abs(info->bmiHeader.biHeight);
    avg_time_per_frame = info->AvgTimePerFrame;
  }

  if ((width <= 0 || height <= 0) && caps) {
    width = std::max<LONG>(caps->InputSize.cx, caps->MinOutputSize.cx);
    height = std::max<LONG>(caps->InputSize.cy, caps->MinOutputSize.cy);
    if (width <= 0 || height <= 0) {
      width = caps->MaxOutputSize.cx;
      height = caps->MaxOutputSize.cy;
    }
  }

  if (width <= 0 || height <= 0) {
    return false;
  }

  out_entry->format = format;
  out_entry->width = static_cast<int>(width);
  out_entry->height = static_cast<int>(height);
  out_entry->fps.clear();

  add_fps_candidate(&out_entry->fps, avg_time_per_frame);
  if (caps) {
    add_fps_candidate(&out_entry->fps, caps->MinFrameInterval);
    add_fps_candidate(&out_entry->fps, caps->MaxFrameInterval);
  }
  normalize_fps(&out_entry->fps);
  return true;
}

void append_stream_capabilities(IAMStreamConfig* stream_config, std::vector<DshowCapabilityEntry>* entries) {
  if (!stream_config || !entries) {
    return;
  }

  int cap_count = 0;
  int cap_size = 0;
  HRESULT hr = stream_config->GetNumberOfCapabilities(&cap_count, &cap_size);
  if (FAILED(hr) || cap_count <= 0 || cap_size < static_cast<int>(sizeof(VIDEO_STREAM_CONFIG_CAPS))) {
    return;
  }

  std::vector<BYTE> caps_buffer(static_cast<size_t>(cap_size));
  for (int index = 0; index < cap_count; ++index) {
    AM_MEDIA_TYPE* media_type = nullptr;
    hr = stream_config->GetStreamCaps(index, &media_type, caps_buffer.data());
    if (FAILED(hr) || !media_type) {
      continue;
    }

    DshowCapabilityEntry entry;
    const auto* caps = reinterpret_cast<const VIDEO_STREAM_CONFIG_CAPS*>(caps_buffer.data());
    if (fill_capability_entry(media_type, caps, &entry)) {
      entries->push_back(std::move(entry));
    }

    free_media_type(media_type);
  }
}

bool find_device_filter(const std::string& device_name, IBaseFilter** out_filter) {
  if (!out_filter) {
    return false;
  }
  *out_filter = nullptr;

  ICreateDevEnum* dev_enum = nullptr;
  IEnumMoniker* enum_moniker = nullptr;
  HRESULT hr = CoCreateInstance(
      CLSID_SystemDeviceEnum,
      nullptr,
      CLSCTX_INPROC_SERVER,
      IID_ICreateDevEnum,
      reinterpret_cast<void**>(&dev_enum));
  if (FAILED(hr) || !dev_enum) {
    return false;
  }

  hr = dev_enum->CreateClassEnumerator(CLSID_VideoInputDeviceCategory, &enum_moniker, 0);
  dev_enum->Release();
  if (hr != S_OK || !enum_moniker) {
    return false;
  }

  bool found = false;
  IMoniker* moniker = nullptr;
  ULONG fetched = 0;
  while (!found && enum_moniker->Next(1, &moniker, &fetched) == S_OK) {
    IPropertyBag* bag = nullptr;
    hr = moniker->BindToStorage(nullptr, nullptr, IID_IPropertyBag, reinterpret_cast<void**>(&bag));
    if (SUCCEEDED(hr) && bag) {
      VARIANT name;
      VariantInit(&name);
      if (SUCCEEDED(bag->Read(L"FriendlyName", &name, nullptr)) && name.vt == VT_BSTR) {
        auto utf8_name = wide_to_utf8(name.bstrVal);
        if (utf8_name == device_name) {
          hr = moniker->BindToObject(nullptr, nullptr, IID_IBaseFilter, reinterpret_cast<void**>(out_filter));
          found = SUCCEEDED(hr) && *out_filter != nullptr;
        }
      }
      VariantClear(&name);
      bag->Release();
    }
    moniker->Release();
  }
  enum_moniker->Release();
  return found;
}

std::string build_capabilities_payload(const std::vector<DshowCapabilityEntry>& entries) {
  std::string payload;
  for (size_t i = 0; i < entries.size(); ++i) {
    const auto& entry = entries[i];
    payload += entry.format;
    payload.push_back('|');
    payload += std::to_string(entry.width);
    payload.push_back('|');
    payload += std::to_string(entry.height);
    payload.push_back('|');
    for (size_t fps_index = 0; fps_index < entry.fps.size(); ++fps_index) {
      payload += std::to_string(entry.fps[fps_index]);
      if (fps_index + 1 < entry.fps.size()) {
        payload.push_back(',');
      }
    }
    if (i + 1 < entries.size()) {
      payload.push_back('\n');
    }
  }
  return payload;
}

char* copy_payload(const std::string& payload) {
  char* out = reinterpret_cast<char*>(std::malloc(payload.size() + 1));
  if (!out) {
    set_last_error("Failed to allocate capture payload buffer");
    return nullptr;
  }
  std::memcpy(out, payload.c_str(), payload.size() + 1);
  return out;
}

int open_dshow_input_with_options(
    AVFormatContext** format_ctx,
    const AVInputFormat* input,
    const std::string& device_name,
    int width,
    int height,
    int fps,
    int requested_format,
    bool use_video_size,
    bool use_framerate,
    bool use_pixel_format,
    std::string* attempt_desc) {
  if (!format_ctx || !input) {
    return AVERROR(EINVAL);
  }

  AVDictionary* options = nullptr;
  std::vector<std::string> parts;

  if (use_video_size && width > 0 && height > 0) {
    std::string video_size = std::to_string(width) + "x" + std::to_string(height);
    av_dict_set(&options, "video_size", video_size.c_str(), 0);
    parts.push_back("video_size=" + video_size);
  }
  if (use_framerate && fps > 0) {
    std::string framerate = std::to_string(fps);
    av_dict_set(&options, "framerate", framerate.c_str(), 0);
    parts.push_back("framerate=" + framerate);
  }

  av_dict_set(&options, "rtbufsize", "64M", 0);
  parts.push_back("rtbufsize=64M");

  const char* pixel_format_name = requested_pixel_format_name(requested_format);
  if (use_pixel_format && pixel_format_name) {
    av_dict_set(&options, "pixel_format", pixel_format_name, 0);
    parts.push_back(std::string("pixel_format=") + pixel_format_name);
  }

  if (attempt_desc) {
    *attempt_desc = parts.empty() ? "default options" : "options{";
    if (!parts.empty()) {
      for (size_t i = 0; i < parts.size(); ++i) {
        if (i > 0) {
          attempt_desc->append(", ");
        }
        attempt_desc->append(parts[i]);
      }
      attempt_desc->append("}");
    }
  }

  std::string input_name = "video=" + device_name;
  int ret = avformat_open_input(format_ctx, input_name.c_str(), input, &options);
  av_dict_free(&options);
  return ret;
}

class ScopedComInit {
 public:
  ScopedComInit() {
    HRESULT hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    initialized_ = hr == S_OK || hr == S_FALSE;
  }

  ~ScopedComInit() {
    if (initialized_) {
      CoUninitialize();
    }
  }

 private:
  bool initialized_ = false;
};

int capture_stride(int pixel_format, int width) {
  switch (pixel_format) {
    case HWCODEC_CAPTURE_FMT_YUYV:
    case HWCODEC_CAPTURE_FMT_YVYU:
    case HWCODEC_CAPTURE_FMT_UYVY:
      return width * 2;
    case HWCODEC_CAPTURE_FMT_RGB24:
    case HWCODEC_CAPTURE_FMT_BGR24:
      return width * 3;
    case HWCODEC_CAPTURE_FMT_NV24:
      return width * 2;
    case HWCODEC_CAPTURE_FMT_NV12:
    case HWCODEC_CAPTURE_FMT_NV21:
    case HWCODEC_CAPTURE_FMT_NV16:
    case HWCODEC_CAPTURE_FMT_YUV420:
    case HWCODEC_CAPTURE_FMT_YVU420:
    case HWCODEC_CAPTURE_FMT_GREY:
    case HWCODEC_CAPTURE_FMT_MJPEG:
    case HWCODEC_CAPTURE_FMT_JPEG:
    default:
      return width;
  }
}

int map_raw_pixfmt(int format) {
  switch (format) {
    case AV_PIX_FMT_YUYV422:
      return HWCODEC_CAPTURE_FMT_YUYV;
    case AV_PIX_FMT_UYVY422:
      return HWCODEC_CAPTURE_FMT_UYVY;
#ifdef AV_PIX_FMT_YVYU422
    case AV_PIX_FMT_YVYU422:
      return HWCODEC_CAPTURE_FMT_YVYU;
#endif
    case AV_PIX_FMT_NV12:
      return HWCODEC_CAPTURE_FMT_NV12;
    case AV_PIX_FMT_NV21:
      return HWCODEC_CAPTURE_FMT_NV21;
#ifdef AV_PIX_FMT_NV16
    case AV_PIX_FMT_NV16:
      return HWCODEC_CAPTURE_FMT_NV16;
#endif
#ifdef AV_PIX_FMT_NV24
    case AV_PIX_FMT_NV24:
      return HWCODEC_CAPTURE_FMT_NV24;
#endif
    case AV_PIX_FMT_YUV420P:
      return HWCODEC_CAPTURE_FMT_YUV420;
#ifdef AV_PIX_FMT_YVU420P
    case AV_PIX_FMT_YVU420P:
      return HWCODEC_CAPTURE_FMT_YVU420;
#endif
    case AV_PIX_FMT_RGB24:
      return HWCODEC_CAPTURE_FMT_RGB24;
    case AV_PIX_FMT_BGR24:
      return HWCODEC_CAPTURE_FMT_BGR24;
    case AV_PIX_FMT_GRAY8:
      return HWCODEC_CAPTURE_FMT_GREY;
    default:
      return HWCODEC_CAPTURE_FMT_UNKNOWN;
  }
}

int map_codec_to_capture_format(const AVCodecParameters* codecpar) {
  if (!codecpar) {
    return HWCODEC_CAPTURE_FMT_UNKNOWN;
  }

  switch (codecpar->codec_id) {
    case AV_CODEC_ID_MJPEG:
      return HWCODEC_CAPTURE_FMT_MJPEG;
    case AV_CODEC_ID_JPEG2000:
      return HWCODEC_CAPTURE_FMT_JPEG;
    case AV_CODEC_ID_RAWVIDEO:
      return map_raw_pixfmt(codecpar->format);
    default:
      return HWCODEC_CAPTURE_FMT_UNKNOWN;
  }
}

int interrupt_callback(void* opaque) {
  auto* ctx = reinterpret_cast<HwcodecDshowCaptureContext*>(opaque);
  if (!ctx) {
    return 0;
  }
  auto deadline = ctx->deadline_ms.load();
  if (deadline <= 0) {
    return 0;
  }
  if (now_ms() > deadline) {
    ctx->timed_out.store(1);
    return 1;
  }
  return 0;
}

const char* requested_pixel_format_name(int requested_format) {
  switch (requested_format) {
    case HWCODEC_CAPTURE_FMT_YUYV:
      return "yuyv422";
    case HWCODEC_CAPTURE_FMT_UYVY:
      return "uyvy422";
    case HWCODEC_CAPTURE_FMT_NV12:
      return "nv12";
    case HWCODEC_CAPTURE_FMT_NV21:
      return "nv21";
    case HWCODEC_CAPTURE_FMT_RGB24:
      return "rgb24";
    case HWCODEC_CAPTURE_FMT_BGR24:
      return "bgr24";
    case HWCODEC_CAPTURE_FMT_GREY:
      return "gray";
    default:
      return nullptr;
  }
}
}  // namespace

extern "C" const char* hwcodec_capture_last_error(void) {
  return g_last_error.c_str();
}

extern "C" char* hwcodec_dshow_list_video_devices(void) {
  ScopedComInit com;

  ICreateDevEnum* dev_enum = nullptr;
  IEnumMoniker* enum_moniker = nullptr;
  HRESULT hr = CoCreateInstance(
      CLSID_SystemDeviceEnum,
      nullptr,
      CLSCTX_INPROC_SERVER,
      IID_ICreateDevEnum,
      reinterpret_cast<void**>(&dev_enum));
  if (FAILED(hr)) {
    set_last_error("Failed to create DirectShow device enumerator");
    return nullptr;
  }

  hr = dev_enum->CreateClassEnumerator(CLSID_VideoInputDeviceCategory, &enum_moniker, 0);
  dev_enum->Release();
  if (hr != S_OK || !enum_moniker) {
    char* out = reinterpret_cast<char*>(std::malloc(1));
    if (out) {
      out[0] = '\0';
    }
    return out;
  }

  std::vector<std::string> devices;
  IMoniker* moniker = nullptr;
  ULONG fetched = 0;
  while (enum_moniker->Next(1, &moniker, &fetched) == S_OK) {
    IPropertyBag* bag = nullptr;
    hr = moniker->BindToStorage(nullptr, nullptr, IID_IPropertyBag, reinterpret_cast<void**>(&bag));
    if (SUCCEEDED(hr) && bag) {
      VARIANT name;
      VariantInit(&name);
      if (SUCCEEDED(bag->Read(L"FriendlyName", &name, nullptr)) && name.vt == VT_BSTR) {
        auto utf8_name = wide_to_utf8(name.bstrVal);
        if (!utf8_name.empty()) {
          devices.push_back(utf8_name);
        }
      }
      VariantClear(&name);
      bag->Release();
    }
    moniker->Release();
  }
  enum_moniker->Release();

  std::string payload;
  for (size_t i = 0; i < devices.size(); ++i) {
    payload += devices[i];
    if (i + 1 < devices.size()) {
      payload.push_back('\n');
    }
  }

  return copy_payload(payload);
}

extern "C" char* hwcodec_dshow_list_device_capabilities(const char* device_name) {
  if (!device_name || device_name[0] == '\0') {
    set_last_error("DirectShow device name is empty");
    return nullptr;
  }

  ScopedComInit com;
  IBaseFilter* filter = nullptr;
  if (!find_device_filter(device_name, &filter) || !filter) {
    set_last_error("Failed to find DirectShow device filter");
    return nullptr;
  }

  std::vector<DshowCapabilityEntry> entries;
  IEnumPins* enum_pins = nullptr;
  HRESULT hr = filter->EnumPins(&enum_pins);
  if (SUCCEEDED(hr) && enum_pins) {
    IPin* pin = nullptr;
    ULONG fetched = 0;
    while (enum_pins->Next(1, &pin, &fetched) == S_OK) {
      PIN_DIRECTION direction = PINDIR_INPUT;
      if (SUCCEEDED(pin->QueryDirection(&direction)) && direction == PINDIR_OUTPUT) {
        IAMStreamConfig* stream_config = nullptr;
        if (SUCCEEDED(pin->QueryInterface(IID_IAMStreamConfig, reinterpret_cast<void**>(&stream_config))) &&
            stream_config) {
          append_stream_capabilities(stream_config, &entries);
          stream_config->Release();
        }
      }
      pin->Release();
    }
    enum_pins->Release();
  }
  filter->Release();

  std::sort(entries.begin(), entries.end(), [](const DshowCapabilityEntry& left, const DshowCapabilityEntry& right) {
    if (left.format != right.format) {
      return left.format < right.format;
    }
    if (left.width != right.width) {
      return left.width < right.width;
    }
    if (left.height != right.height) {
      return left.height < right.height;
    }
    return left.fps > right.fps;
  });
  entries.erase(
      std::unique(entries.begin(), entries.end(), [](const DshowCapabilityEntry& left, const DshowCapabilityEntry& right) {
        return left.format == right.format && left.width == right.width && left.height == right.height && left.fps == right.fps;
      }),
      entries.end());

  return copy_payload(build_capabilities_payload(entries));
}

extern "C" void hwcodec_capture_string_free(char* ptr) {
  if (ptr) {
    std::free(ptr);
  }
}

extern "C" HwcodecDshowCaptureContext* hwcodec_dshow_capture_open(
    const char* device_name,
    int width,
    int height,
    int fps,
    int requested_format,
    int timeout_ms) {
  if (!device_name || device_name[0] == '\0') {
    set_last_error("Device name is empty");
    return nullptr;
  }

  avdevice_register_all();

  const AVInputFormat* input = av_find_input_format("dshow");
  if (!input) {
    set_last_error("FFmpeg dshow input format is unavailable");
    return nullptr;
  }

  auto* ctx = new HwcodecDshowCaptureContext();
  ctx->timeout_ms = timeout_ms > 0 ? timeout_ms : 2000;
  ctx->format_ctx = avformat_alloc_context();
  if (!ctx->format_ctx) {
    delete ctx;
    set_last_error("Failed to allocate FFmpeg format context");
    return nullptr;
  }
  ctx->format_ctx->interrupt_callback.callback = interrupt_callback;
  ctx->format_ctx->interrupt_callback.opaque = ctx;

  std::string open_attempt;
  int ret = open_dshow_input_with_options(
      &ctx->format_ctx,
      input,
      device_name,
      width,
      height,
      fps,
      requested_format,
      true,
      true,
      true,
      &open_attempt);

  if (ret < 0) {
    avformat_free_context(ctx->format_ctx);
    ctx->format_ctx = avformat_alloc_context();
    if (!ctx->format_ctx) {
      delete ctx;
      set_last_error("Failed to allocate FFmpeg format context for fallback open");
      return nullptr;
    }
    ctx->format_ctx->interrupt_callback.callback = interrupt_callback;
    ctx->format_ctx->interrupt_callback.opaque = ctx;

    std::string fallback_attempt;
    ret = open_dshow_input_with_options(
        &ctx->format_ctx,
        input,
        device_name,
        width,
        height,
        fps,
        requested_format,
        true,
        false,
        true,
        &fallback_attempt);
    if (ret >= 0) {
      open_attempt = fallback_attempt;
    }
  }

  if (ret < 0) {
    avformat_free_context(ctx->format_ctx);
    ctx->format_ctx = avformat_alloc_context();
    if (!ctx->format_ctx) {
      delete ctx;
      set_last_error("Failed to allocate FFmpeg format context for final fallback open");
      return nullptr;
    }
    ctx->format_ctx->interrupt_callback.callback = interrupt_callback;
    ctx->format_ctx->interrupt_callback.opaque = ctx;

    std::string fallback_attempt;
    ret = open_dshow_input_with_options(
        &ctx->format_ctx,
        input,
        device_name,
        width,
        height,
        fps,
        requested_format,
        false,
        false,
        false,
        &fallback_attempt);
    if (ret >= 0) {
      open_attempt = fallback_attempt;
    }
  }

  if (ret < 0) {
    set_last_error("Failed to open dshow input (" + open_attempt + "): " + ffmpeg_error(ret));
    avformat_free_context(ctx->format_ctx);
    delete ctx;
    return nullptr;
  }

  ret = avformat_find_stream_info(ctx->format_ctx, nullptr);
  if (ret < 0) {
    set_last_error("Failed to read stream info: " + ffmpeg_error(ret));
    avformat_close_input(&ctx->format_ctx);
    delete ctx;
    return nullptr;
  }

  for (unsigned int i = 0; i < ctx->format_ctx->nb_streams; ++i) {
    AVStream* stream = ctx->format_ctx->streams[i];
    if (stream && stream->codecpar && stream->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
      ctx->stream_index = static_cast<int>(i);
      ctx->width = stream->codecpar->width > 0 ? stream->codecpar->width : width;
      ctx->height = stream->codecpar->height > 0 ? stream->codecpar->height : height;
      ctx->pixel_format = map_codec_to_capture_format(stream->codecpar);
      ctx->stride = capture_stride(ctx->pixel_format, ctx->width);
      break;
    }
  }

  if (ctx->stream_index < 0) {
    set_last_error("No video stream found on DirectShow device");
    avformat_close_input(&ctx->format_ctx);
    delete ctx;
    return nullptr;
  }

  if (ctx->pixel_format == HWCODEC_CAPTURE_FMT_UNKNOWN) {
    set_last_error("DirectShow stream format is unsupported in current Windows backend");
    avformat_close_input(&ctx->format_ctx);
    delete ctx;
    return nullptr;
  }

  return ctx;
}

extern "C" int hwcodec_dshow_capture_info(
    HwcodecDshowCaptureContext* ctx,
    HwcodecCaptureStreamInfo* out_info) {
  if (!ctx || !out_info) {
    set_last_error("Invalid capture context");
    return -1;
  }

  out_info->width = ctx->width;
  out_info->height = ctx->height;
  out_info->pixel_format = ctx->pixel_format;
  out_info->stride = ctx->stride;
  return 0;
}

extern "C" int hwcodec_dshow_capture_read(
    HwcodecDshowCaptureContext* ctx,
    uint8_t** out_data,
    int* out_len,
    uint64_t* out_sequence) {
  if (!ctx || !out_data || !out_len || !out_sequence) {
    set_last_error("Invalid capture read arguments");
    return -1;
  }

  *out_data = nullptr;
  *out_len = 0;
  *out_sequence = 0;

  AVPacket packet;
  av_init_packet(&packet);
  packet.data = nullptr;
  packet.size = 0;

  while (true) {
    ctx->timed_out.store(0);
    ctx->deadline_ms.store(now_ms() + ctx->timeout_ms);
    int ret = av_read_frame(ctx->format_ctx, &packet);
    ctx->deadline_ms.store(0);

    if (ret < 0) {
      if (ctx->timed_out.load() != 0) {
        set_last_error("Timed out waiting for frame");
        return -110;
      }
      set_last_error("Failed to read frame: " + ffmpeg_error(ret));
      return ret;
    }

    if (packet.stream_index != ctx->stream_index) {
      av_packet_unref(&packet);
      continue;
    }

    if (packet.size <= 0 || !packet.data) {
      av_packet_unref(&packet);
      continue;
    }

    auto* buffer = reinterpret_cast<uint8_t*>(std::malloc(static_cast<size_t>(packet.size)));
    if (!buffer) {
      av_packet_unref(&packet);
      set_last_error("Failed to allocate packet buffer");
      return -12;
    }

    std::memcpy(buffer, packet.data, static_cast<size_t>(packet.size));
    *out_data = buffer;
    *out_len = packet.size;
    *out_sequence = ctx->sequence++;
    av_packet_unref(&packet);
    return 0;
  }
}

extern "C" void hwcodec_dshow_capture_packet_free(uint8_t* data) {
  if (data) {
    std::free(data);
  }
}

extern "C" void hwcodec_dshow_capture_close(HwcodecDshowCaptureContext* ctx) {
  if (!ctx) {
    return;
  }
  if (ctx->format_ctx) {
    avformat_close_input(&ctx->format_ctx);
  }
  delete ctx;
}
