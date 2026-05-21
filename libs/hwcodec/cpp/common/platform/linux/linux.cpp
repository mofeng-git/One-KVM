#include "linux.h"
#include "../../log.h"
#include <algorithm>
#include <cctype>
#include <cstring>
#include <dlfcn.h>
#include <errno.h>
#include <fstream>
#include <signal.h>
#include <sys/prctl.h>
#include <unistd.h>
#include <fcntl.h>
#include <string>

// Check for NVIDIA driver support by loading CUDA libraries
int linux_support_nv()
{
  // Try to load NVIDIA CUDA runtime library
  void *handle = dlopen("libcuda.so.1", RTLD_LAZY);
  if (!handle)
  {
    handle = dlopen("libcuda.so", RTLD_LAZY);
  }
  if (!handle)
  {
    LOG_TRACE(std::string("NVIDIA: libcuda.so not found"));
    return -1;
  }
  dlclose(handle);

  // Also check for nvenc library
  handle = dlopen("libnvidia-encode.so.1", RTLD_LAZY);
  if (!handle)
  {
    handle = dlopen("libnvidia-encode.so", RTLD_LAZY);
  }
  if (!handle)
  {
    LOG_TRACE(std::string("NVIDIA: libnvidia-encode.so not found"));
    return -1;
  }
  dlclose(handle);

  LOG_TRACE(std::string("NVIDIA: driver support detected"));
  return 0;
}

int linux_support_amd()
{
#if defined(__x86_64__) || defined(__aarch64__)
#define AMF_DLL_NAMEA "libamfrt64.so.1"
#else
#define AMF_DLL_NAMEA "libamfrt32.so.1"
#endif
  void *handle = dlopen(AMF_DLL_NAMEA, RTLD_LAZY);
  if (!handle)
  {
    return -1;
  }
  dlclose(handle);
  return 0;
}

int linux_support_intel()
{
  const char *libs[] =
      {"libvpl.so", "libmfx.so", "libmfx-gen.so.1.2", "libmfxhw64.so.1"};
  for (size_t i = 0; i < sizeof(libs) / sizeof(libs[0]); i++)
  {
    void *handle = dlopen(libs[i], RTLD_LAZY);
    if (handle)
    {
      dlclose(handle);
      return 0;
    }
  }
  return -1;
}

int setup_parent_death_signal() {
  // Set up parent death signal to ensure this process dies if parent dies
  // This prevents orphaned processes especially when running with different
  // user permissions
  int ret = prctl(PR_SET_PDEATHSIG, SIGKILL);
  if (ret != 0) {
    LOG_ERROR(std::string("Failed to set parent death signal:") + std::to_string(errno));
    return -1;
  } else {
    return 0;
  }
}

// Check for Rockchip MPP (Media Process Platform) support
// Returns 0 if supported, -1 otherwise
int linux_support_rkmpp() {
  // Check for MPP service device (primary method)
  if (access("/dev/mpp_service", F_OK) == 0) {
    LOG_TRACE(std::string("RKMPP: Found /dev/mpp_service"));
    return 0;
  }
  // Fallback: check for RGA (Rockchip Graphics Acceleration) device
  if (access("/dev/rga", F_OK) == 0) {
    LOG_TRACE(std::string("RKMPP: Found /dev/rga"));
    return 0;
  }
  LOG_TRACE(std::string("RKMPP: No Rockchip MPP device found"));
  return -1;
}

// Check for V4L2 Memory-to-Memory (M2M) codec support
// Returns 0 if a M2M capable device is found, -1 otherwise
int linux_support_v4l2m2m() {
  auto to_lower = [](std::string value) {
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char c) {
      return static_cast<char>(std::tolower(c));
    });
    return value;
  };

  auto read_text_file = [](const char *path, std::string *out) -> bool {
    std::ifstream file(path);
    if (!file.is_open()) {
      return false;
    }
    std::getline(file, *out, '\0');
    return !out->empty();
  };

  auto v4l2m2m_allowed = []() -> bool {
    const char *env = std::getenv("ONE_KVM_V4L2M2M_ALLOW");
    if (env == nullptr) {
      return false;
    }
    if (env[0] == '\0') {
      return false;
    }
    return std::strcmp(env, "0") != 0;
  };

  auto contains_any = [](const std::string &value, const char *const *needles, size_t len) -> bool {
    for (size_t i = 0; i < len; i++) {
      if (value.find(needles[i]) != std::string::npos) {
        return true;
      }
    }
    return false;
  };

  auto is_amlogic_platform = [&]() -> bool {
    const char *platform_hints[] = {
      "amlogic",
      "meson",
      "gxl",
      "gxbb",
      "gxm",
      "g12a",
      "g12b",
      "sm1",
    };

    const char *platform_files[] = {
      "/proc/device-tree/compatible",
      "/proc/device-tree/model",
      "/sys/firmware/devicetree/base/compatible",
      "/sys/firmware/devicetree/base/model",
    };

    for (size_t i = 0; i < sizeof(platform_files) / sizeof(platform_files[0]); i++) {
      std::string value;
      if (read_text_file(platform_files[i], &value) &&
          contains_any(to_lower(value), platform_hints,
                       sizeof(platform_hints) / sizeof(platform_hints[0]))) {
        return true;
      }
    }

    const char *video_nodes[] = {
      "video0",
      "video1",
      "video2",
      "video10",
      "video11",
      "video32",
    };
    const char *vdec_hints[] = {
      "meson",
      "amlogic",
      "vdec",
      "decoder",
      "video-decoder",
      "gxl-vdec",
      "gx-vdec",
    };

    for (size_t i = 0; i < sizeof(video_nodes) / sizeof(video_nodes[0]); i++) {
      std::string name;
      std::string modalias;
      const std::string base = std::string("/sys/class/video4linux/") + video_nodes[i];
      if (read_text_file((base + "/name").c_str(), &name) &&
          contains_any(to_lower(name), vdec_hints, sizeof(vdec_hints) / sizeof(vdec_hints[0]))) {
        return true;
      }
      if (read_text_file((base + "/device/modalias").c_str(), &modalias) &&
          contains_any(to_lower(modalias), vdec_hints,
                       sizeof(vdec_hints) / sizeof(vdec_hints[0]))) {
        return true;
      }
    }

    return false;
  };

  const bool amlogic_platform = is_amlogic_platform();
  if (amlogic_platform && !v4l2m2m_allowed()) {
    LOG_WARN(std::string(
        "V4L2 M2M: skipped probe on Amlogic platform; set ONE_KVM_V4L2M2M_ALLOW=1 to enable"));
    return -1;
  }

  if (amlogic_platform) {
    LOG_WARN(std::string("V4L2 M2M: ONE_KVM_V4L2M2M_ALLOW is set; probing Amlogic video nodes"));
  }

  // Check common V4L2 M2M device paths used by various ARM SoCs
  // /dev/video10 - Standard on many SoCs
  // /dev/video11 - Standard on many SoCs (often decoder)
  // /dev/video0 - Some platforms (like RPi) might use this
  // /dev/video1 - Alternate RPi path
  // /dev/video2 - Alternate path
  // /dev/video32 - Some Allwinner/Rockchip legacy
  const char *m2m_devices[] = {
    "/dev/video10",
    "/dev/video11",
    "/dev/video0",
    "/dev/video1",
    "/dev/video2",
    "/dev/video32",
  };

  for (size_t i = 0; i < sizeof(m2m_devices) / sizeof(m2m_devices[0]); i++) {
    if (access(m2m_devices[i], F_OK) == 0) {
      // Device exists, check if it's an M2M device by trying to open it
      int fd = open(m2m_devices[i], O_RDWR | O_NONBLOCK);
      if (fd >= 0) {
        close(fd);
        LOG_TRACE(std::string("V4L2 M2M: Found device ") + m2m_devices[i]);
        return 0;
      }
    }
  }

  LOG_TRACE(std::string("V4L2 M2M: No M2M device found"));
  return -1;
}
