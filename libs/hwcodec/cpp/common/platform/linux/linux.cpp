#include "linux.h"
#include "../../log.h"
#include <cstring>
#include <dlfcn.h>
#include <errno.h>
#include <signal.h>
#include <sys/prctl.h>
#include <unistd.h>
#include <fcntl.h>

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
  // Check common V4L2 M2M device paths used by various ARM SoCs
  const char *m2m_devices[] = {
    "/dev/video10",  // Common M2M encoder device
    "/dev/video11",  // Common M2M decoder device
    "/dev/video0",   // Some SoCs use video0 for M2M
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
