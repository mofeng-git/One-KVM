/******************************************************************************\
Copyright (c) 2005-2019, Intel Corporation
All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

This sample was distributed or derived from the Intel's Media Samples package.
The original version of this sample may be obtained from https://software.intel.com/en-us/intel-media-server-studio
or https://software.intel.com/en-us/media-client-solutions-support.
\**********************************************************************************/

#if defined(LIBVA_DRM_SUPPORT) || defined(LIBVA_WAYLAND_SUPPORT)

#include "vaapi_utils_drm.h"
#include "vaapi_allocator.h"
#include <fcntl.h>
#include <sys/ioctl.h>

#include <stdexcept>

#include "vaapi_utils_drm.h"
#include <drm_fourcc.h>
#include "i915_drm.h"

constexpr mfxU32 MFX_DRI_MAX_NODES_NUM = 16;
constexpr mfxU32 MFX_DRI_RENDER_START_INDEX = 128;
constexpr mfxU32 MFX_DRI_CARD_START_INDEX = 0;
constexpr  mfxU32 MFX_DRM_DRIVER_NAME_LEN = 4;
const char* MFX_DRM_INTEL_DRIVER_NAME = "i915";
const char* MFX_DRI_PATH = "/dev/dri/";
const char* MFX_DRI_NODE_RENDER = "renderD";
const char* MFX_DRI_NODE_CARD = "card";

int get_drm_driver_name(int fd, char *name, int name_size)
{
    drm_version_t version = {};
    version.name_len = name_size;
    version.name = name;
    return ioctl(fd, DRM_IOWR(0, drm_version), &version);
}

int open_first_intel_adapter(int type)
{
    std::string adapterPath = MFX_DRI_PATH;
    char driverName[MFX_DRM_DRIVER_NAME_LEN + 1] = {};
    mfxU32 nodeIndex;

    switch (type) {
    case MFX_LIBVA_DRM:
    case MFX_LIBVA_AUTO:
        adapterPath += MFX_DRI_NODE_RENDER;
        nodeIndex = MFX_DRI_RENDER_START_INDEX;
        break;
    case MFX_LIBVA_DRM_MODESET:
        adapterPath += MFX_DRI_NODE_CARD;
        nodeIndex = MFX_DRI_CARD_START_INDEX;
        break;
    default:
        throw std::invalid_argument("Wrong libVA backend type");
    }

    for (mfxU32 i = 0; i < MFX_DRI_MAX_NODES_NUM; ++i) {
        std::string curAdapterPath = adapterPath + std::to_string(nodeIndex + i);

        int fd = open(curAdapterPath.c_str(), O_RDWR);
        if (fd < 0) continue;

        if (!get_drm_driver_name(fd, driverName, MFX_DRM_DRIVER_NAME_LEN) &&
            !strcmp(driverName, MFX_DRM_INTEL_DRIVER_NAME)) {
            return fd;
        }
        close(fd);
    }

    return -1;
}

int open_intel_adapter(const std::string& devicePath, int type)
{
    if(devicePath.empty())
        return open_first_intel_adapter(type);

    int fd = open(devicePath.c_str(), O_RDWR);

    if (fd < 0) {
        msdk_printf(MSDK_STRING("Failed to open specified device\n"));
        return -1;
    }

    char driverName[MFX_DRM_DRIVER_NAME_LEN + 1] = {};
    if (!get_drm_driver_name(fd, driverName, MFX_DRM_DRIVER_NAME_LEN) &&
        !strcmp(driverName, MFX_DRM_INTEL_DRIVER_NAME)) {
            return fd;
    }
    else {
        close(fd);
        msdk_printf(MSDK_STRING("Specified device is not Intel one\n"));
        return -1;
    }
}

DRMLibVA::DRMLibVA(const std::string& devicePath, int type)
    : CLibVA(type)
    , m_fd(-1)
{
    mfxStatus sts = MFX_ERR_NONE;

    m_fd = open_intel_adapter(devicePath, type);
    if (m_fd < 0) throw std::range_error("Intel GPU was not found");

    m_va_dpy = m_vadrmlib.vaGetDisplayDRM(m_fd);
    if (m_va_dpy)
    {
        int major_version = 0, minor_version = 0;
        VAStatus va_res = m_libva.vaInitialize(m_va_dpy, &major_version, &minor_version);
        sts = va_to_mfx_status(va_res);
    }
    else {
        sts = MFX_ERR_NULL_PTR;
    }

    if (MFX_ERR_NONE != sts)
    {
        if (m_va_dpy) m_libva.vaTerminate(m_va_dpy);
        close(m_fd);
        throw std::runtime_error("Loading of VA display was failed");
    }
}

DRMLibVA::~DRMLibVA(void)
{
    if (m_va_dpy)
    {
        m_libva.vaTerminate(m_va_dpy);
    }
    if (m_fd >= 0)
    {
        close(m_fd);
    }
}

struct drmMonitorsTable {
  mfxI32 mfx_type;
  uint32_t drm_type;
  const msdk_char * type_name;
};

drmMonitorsTable g_drmMonitorsTable[] = {
#define __DECLARE(type) { MFX_MONITOR_ ## type, DRM_MODE_CONNECTOR_ ## type, MSDK_STRING(#type) }
  __DECLARE(Unknown),
  __DECLARE(VGA),
  __DECLARE(DVII),
  __DECLARE(DVID),
  __DECLARE(DVIA),
  __DECLARE(Composite),
  __DECLARE(SVIDEO),
  __DECLARE(LVDS),
  __DECLARE(Component),
  __DECLARE(9PinDIN),
  __DECLARE(HDMIA),
  __DECLARE(HDMIB),
  __DECLARE(eDP),
  __DECLARE(TV),
  __DECLARE(DisplayPort),
#if defined(DRM_MODE_CONNECTOR_VIRTUAL) // from libdrm 2.4.59
  __DECLARE(VIRTUAL),
#endif
#if defined(DRM_MODE_CONNECTOR_DSI) // from libdrm 2.4.59
  __DECLARE(DSI)
#endif
#undef __DECLARE
};

uint32_t drmRenderer::getConnectorType(mfxI32 monitor_type)
{
    for (size_t i=0; i < sizeof(g_drmMonitorsTable)/sizeof(g_drmMonitorsTable[0]); ++i) {
      if (g_drmMonitorsTable[i].mfx_type == monitor_type) {
        return g_drmMonitorsTable[i].drm_type;
      }
    }
    return DRM_MODE_CONNECTOR_Unknown;
}

const msdk_char* drmRenderer::getConnectorName(uint32_t connector_type)
{
    for (size_t i=0; i < sizeof(g_drmMonitorsTable)/sizeof(g_drmMonitorsTable[0]); ++i) {
      if (g_drmMonitorsTable[i].drm_type == connector_type) {
        return g_drmMonitorsTable[i].type_name;
      }
    }
    return MSDK_STRING("Unknown");
}

drmRenderer::drmRenderer(int fd, mfxI32 monitorType)
  : m_fd(fd)
  , m_bufmgr(NULL)
  , m_overlay_wrn(true)
  , m_pCurrentRenderTargetSurface(NULL)
{
    bool res = false;
    uint32_t connectorType = getConnectorType(monitorType);

    if (monitorType == MFX_MONITOR_AUTO) {
      connectorType = DRM_MODE_CONNECTOR_Unknown;
    } else if (connectorType == DRM_MODE_CONNECTOR_Unknown) {
      throw std::invalid_argument("Unsupported monitor type");
    }
    drmModeRes *resource = m_drmlib.drmModeGetResources(m_fd);
    if (resource) {
        if (getConnector(resource, connectorType) &&
            getPlane()) {
            res = true;
        }
        m_drmlib.drmModeFreeResources(resource);
    }
    if (!res) {
      throw std::invalid_argument("Failed to allocate renderer");
    }
    msdk_printf(MSDK_STRING("drmrender: connected via %s to %dx%d@%d capable display\n"),
      getConnectorName(m_connector_type), m_mode.hdisplay, m_mode.vdisplay, m_mode.vrefresh);
}

drmRenderer::~drmRenderer()
{
    m_drmlib.drmModeFreeCrtc(m_crtc);
    if (m_bufmgr)
    {
        m_drmintellib.drm_intel_bufmgr_destroy(m_bufmgr);
        m_bufmgr = NULL;
    }
}

bool drmRenderer::getConnector(drmModeRes *resource, uint32_t connector_type)
{
    bool found = false;
    drmModeConnectorPtr connector = NULL;

    for (int i = 0; i < resource->count_connectors; ++i) {
        connector = m_drmlib.drmModeGetConnector(m_fd, resource->connectors[i]);
        if (connector) {
            if ((connector->connector_type == connector_type) ||
                (connector_type == DRM_MODE_CONNECTOR_Unknown)) {
                if (connector->connection == DRM_MODE_CONNECTED) {
                    msdk_printf(MSDK_STRING("drmrender: trying connection: %s\n"), getConnectorName(connector->connector_type));
                    m_connector_type = connector->connector_type;
                    m_connectorID = connector->connector_id;
                    found = setupConnection(resource, connector);
                    if (found) msdk_printf(MSDK_STRING("drmrender: succeeded...\n"));
                    else msdk_printf(MSDK_STRING("drmrender: failed...\n"));
                } else if ((connector_type != DRM_MODE_CONNECTOR_Unknown)) {
                    msdk_printf(MSDK_STRING("drmrender: error: requested monitor not connected\n"));
                }
            }
            m_drmlib.drmModeFreeConnector(connector);
            if (found) return true;
        }
    }
    msdk_printf(MSDK_STRING("drmrender: error: requested monitor not available\n"));
    return false;
}

bool drmRenderer::setupConnection(drmModeRes *resource, drmModeConnector* connector)
{
    bool ret = false;
    drmModeEncoderPtr encoder;

    if (!connector->count_modes) {
      msdk_printf(MSDK_STRING("drmrender: error: no valid modes for %s connector\n"),
        getConnectorName(connector->connector_type));
      return false;
    }
    // we will use the first available mode - that's always mode with the highest resolution
    m_mode = connector->modes[0];

    // trying encoder+crtc which are currently attached to connector
    m_encoderID = connector->encoder_id;
    encoder = m_drmlib.drmModeGetEncoder(m_fd, m_encoderID);
    if (encoder) {
      m_crtcID = encoder->crtc_id;
      for (int j = 0; j < resource->count_crtcs; ++j)
      {
          if (m_crtcID == resource->crtcs[j])
          {
              m_crtcIndex = j;
              break;
          }
      }
      ret = true;
      msdk_printf(MSDK_STRING("drmrender: selected crtc already attached to connector\n"));
      m_drmlib.drmModeFreeEncoder(encoder);
    }

    // if previous attempt to get crtc failed, let performs global search
    // searching matching encoder+crtc globally
    if (!ret) {
      for (int i = 0; i < connector->count_encoders; ++i) {
        encoder = m_drmlib.drmModeGetEncoder(m_fd, connector->encoders[i]);
        if (encoder) {
          for (int j = 0; j < resource->count_crtcs; ++j) {
            // check whether this CRTC works with the encoder
            if ( !((encoder->possible_crtcs & (1 << j)) &&
                   (encoder->crtc_id == resource->crtcs[j])) )
                continue;

            m_encoderID = connector->encoders[i];
            m_crtcIndex = j;
            m_crtcID = resource->crtcs[j];
            ret = true;
            msdk_printf(MSDK_STRING("drmrender: found crtc with global search\n"));
            break;
          }
          m_drmlib.drmModeFreeEncoder(encoder);
          if (ret)
              break;
        }
      }
    }
    if (ret) {
        m_crtc = m_drmlib.drmModeGetCrtc(m_fd, m_crtcID);
        if (!m_crtc)
            ret = false;
    } else {
        msdk_printf(MSDK_STRING("drmrender: failed to select crtc\n"));
    }
    return ret;
}

bool drmRenderer::getPlane()
{
    drmModePlaneResPtr planes = m_drmlib.drmModeGetPlaneResources(m_fd);
    if (!planes) {
        return false;
    }
    for (uint32_t i = 0; i < planes->count_planes; ++i) {
        drmModePlanePtr plane = m_drmlib.drmModeGetPlane(m_fd, planes->planes[i]);
        if (plane) {
            if (plane->possible_crtcs & (1 << m_crtcIndex)) {
                for (uint32_t j = 0; j < plane->count_formats; ++j) {
                    if ((plane->formats[j] == DRM_FORMAT_XRGB8888)
                        || (plane->formats[j] == DRM_FORMAT_NV12)) {
                        m_planeID = plane->plane_id;
                        m_drmlib.drmModeFreePlane(plane);
                        m_drmlib.drmModeFreePlaneResources(planes);
                        return true;
                    }
                }
            }
            m_drmlib.drmModeFreePlane(plane);
        }
    }
    m_drmlib.drmModeFreePlaneResources(planes);
    return false;
}

bool drmRenderer::setMaster()
{
    int wait_count = 0;
    do {
      if (!m_drmlib.drmSetMaster(m_fd)) return true;
      usleep(100);
      ++wait_count;
    } while(wait_count < 30000);
    msdk_printf(MSDK_STRING("drmrender: error: failed to get drm mastership during 3 seconds - aborting\n"));
    return false;
}

void drmRenderer::dropMaster()
{
    m_drmlib.drmDropMaster(m_fd);
}

bool drmRenderer::restore()
{
  if (!setMaster()) return false;

  int ret = m_drmlib.drmModeSetCrtc(m_fd, m_crtcID, m_crtc->buffer_id, m_crtc->x, m_crtc->y, &m_connectorID, 1, &m_mode);
  if (ret) {
    msdk_printf(MSDK_STRING("drmrender: failed to restore original mode\n"));
    return false;
  }
  dropMaster();
  return true;
}

void* drmRenderer::acquire(mfxMemId mid)
{
    vaapiMemId* vmid = (vaapiMemId*)mid;
    uint32_t fbhandle=0;

    if (vmid->m_buffer_info.mem_type == VA_SURFACE_ATTRIB_MEM_TYPE_DRM_PRIME) {
        if (!m_bufmgr) {
            m_bufmgr = m_drmintellib.drm_intel_bufmgr_gem_init(m_fd, 4096);
            if (!m_bufmgr) return NULL;
        }

        drm_intel_bo* bo = m_drmintellib.drm_intel_bo_gem_create_from_prime(
            m_bufmgr, (int)vmid->m_buffer_info.handle, vmid->m_buffer_info.mem_size);
        if (!bo) return NULL;

        int ret = m_drmlib.drmModeAddFB(m_fd,
              vmid->m_image.width, vmid->m_image.height,
              24, 32, vmid->m_image.pitches[0],
              bo->handle, &fbhandle);
        if (ret) {
          return NULL;
        }
        m_drmintellib.drm_intel_bo_unreference(bo);
    } else if (vmid->m_buffer_info.mem_type == VA_SURFACE_ATTRIB_MEM_TYPE_KERNEL_DRM) {
        struct drm_gem_open flink_open;
        struct drm_gem_close flink_close;

        MSDK_ZERO_MEMORY(flink_open);
        flink_open.name = vmid->m_buffer_info.handle;
        int ret = m_drmlib.drmIoctl(m_fd, DRM_IOCTL_GEM_OPEN, &flink_open);
        if (ret) return NULL;

        uint32_t handles[4], pitches[4], offsets[4], pixel_format, flags = 0;
        uint64_t modifiers[4];

        memset(&handles, 0, sizeof(handles));
        memset(&pitches, 0, sizeof(pitches));
        memset(&offsets, 0, sizeof(offsets));
        memset(&modifiers, 0, sizeof(modifiers));

        handles[0] = flink_open.handle;
        pitches[0] = vmid->m_image.pitches[0];
        offsets[0] = vmid->m_image.offsets[0];

        if (VA_FOURCC_NV12 == vmid->m_fourcc) {
            struct drm_i915_gem_set_tiling set_tiling;

            pixel_format = DRM_FORMAT_NV12;
            memset(&set_tiling, 0, sizeof(set_tiling));
            set_tiling.handle = flink_open.handle;
            set_tiling.tiling_mode = I915_TILING_Y;
            set_tiling.stride = vmid->m_image.pitches[0];
            ret = m_drmlib.drmIoctl(m_fd, DRM_IOCTL_I915_GEM_SET_TILING, &set_tiling);
            if (ret) {
                msdk_printf(MSDK_STRING("DRM_IOCTL_I915_GEM_SET_TILING Failed ret = %d\n"),ret);
                return NULL;
            }

            handles[1] = flink_open.handle;
            pitches[1] = vmid->m_image.pitches[1];
            offsets[1] = vmid->m_image.offsets[1];
            modifiers[0] = modifiers[1] = I915_FORMAT_MOD_Y_TILED;
            flags = 2; // DRM_MODE_FB_MODIFIERS   (1<<1) /* enables ->modifer[]
       }
       else {
            pixel_format = DRM_FORMAT_XRGB8888;
       }

        ret = m_drmlib.drmModeAddFB2WithModifiers(m_fd, vmid->m_image.width, vmid->m_image.height,
              pixel_format, handles, pitches, offsets, modifiers, &fbhandle, flags);
        if (ret) return NULL;

        MSDK_ZERO_MEMORY(flink_close);
        flink_close.handle = flink_open.handle;
        ret = m_drmlib.drmIoctl(m_fd, DRM_IOCTL_GEM_CLOSE, &flink_close);
        if (ret) return NULL;
    } else {
        return NULL;
    }
    try {
        uint32_t* hdl = new uint32_t;
        *hdl = fbhandle;
        return hdl;
    } catch(...) {
        return NULL;
    }
}

void drmRenderer::release(mfxMemId mid, void * mem)
{
    uint32_t* hdl = (uint32_t*)mem;
    if (!hdl) return;
    if (!restore()) {
      msdk_printf(MSDK_STRING("drmrender: warning: failure to restore original mode may lead to application segfault!\n"));
    }
    m_drmlib.drmModeRmFB(m_fd, *hdl);
    delete(hdl);
}

mfxStatus drmRenderer::render(mfxFrameSurface1 * pSurface)
{
    int ret;
    vaapiMemId * memid;
    uint32_t fbhandle;

    if (!pSurface || !pSurface->Data.MemId) return MFX_ERR_INVALID_HANDLE;
    memid = (vaapiMemId*)(pSurface->Data.MemId);
    if (!memid->m_custom) return MFX_ERR_INVALID_HANDLE;
    fbhandle = *(uint32_t*)memid->m_custom;

    // rendering on the screen
    if (!setMaster()) {
      return MFX_ERR_UNKNOWN;
    }
    if ((m_mode.hdisplay == memid->m_image.width) &&
        (m_mode.vdisplay == memid->m_image.height)) {
        // surface in the framebuffer exactly matches crtc scanout port, so we
        // can scanout from this framebuffer for the whole crtc
        ret = m_drmlib.drmModeSetCrtc(m_fd, m_crtcID, fbhandle, 0, 0, &m_connectorID, 1, &m_mode);
        if (ret) {
          return MFX_ERR_UNKNOWN;
        }
    } else {
        if (m_overlay_wrn) {
          m_overlay_wrn = false;
          msdk_printf(MSDK_STRING("drmrender: warning: rendering via OVERLAY plane\n"));
        }
        // surface in the framebuffer exactly does NOT match crtc scanout port,
        // and we can only use overlay technique with possible resize (depending on the driver))
        ret = m_drmlib.drmModeSetPlane(m_fd, m_planeID, m_crtcID, fbhandle, 0,
          0, 0, m_crtc->width, m_crtc->height,
          pSurface->Info.CropX << 16, pSurface->Info.CropY << 16, pSurface->Info.CropW << 16, pSurface->Info.CropH << 16);
        if (ret) {
          return MFX_ERR_UNKNOWN;
        }
    }
    dropMaster();

    /* Unlock previous Render Target Surface (if exists) */
    if (NULL != m_pCurrentRenderTargetSurface)
        msdk_atomic_dec16((volatile mfxU16*)&m_pCurrentRenderTargetSurface->Data.Locked);

    /* new Render target */
    m_pCurrentRenderTargetSurface = pSurface;
    /* And lock it */
    msdk_atomic_inc16((volatile mfxU16*)&m_pCurrentRenderTargetSurface->Data.Locked);
    return MFX_ERR_NONE;
}

#endif // #if defined(LIBVA_DRM_SUPPORT)
