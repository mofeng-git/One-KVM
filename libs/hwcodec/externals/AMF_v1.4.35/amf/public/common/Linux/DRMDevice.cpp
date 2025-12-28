//
// Notice Regarding Standards.  AMD does not provide a license or sublicense to
// any Intellectual Property Rights relating to any standards, including but not
// limited to any audio and/or video codec technologies such as MPEG-2, MPEG-4;
// AVC/H.264; HEVC/H.265; AAC decode/FFMPEG; AAC encode/FFMPEG; VC-1; AV1; and MP3
// (collectively, the "Media Technologies"). For clarity, you will pay any
// royalties due for such third party technologies, which may include the Media
// Technologies that are owed as a result of AMD providing the Software to you.
//
// MIT license
//
// Copyright (c) 2023 Advanced Micro Devices, Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//
#include "DRMDevice.h"
#include <drm.h>
#include <drm_fourcc.h>
#include <drm_mode.h>
#include <amdgpu_drm.h>
#include <xf86drm.h>
#include <xf86drmMode.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <dirent.h>
#include <fcntl.h>
#include <unistd.h>

#define AMF_FACILITY L"DRMDevice"

struct FormatMapEntry
{
    amf::AMF_SURFACE_FORMAT formatAMF;
    uint32_t                formatDRM;
};
static const FormatMapEntry formatMap [] =
{
#ifdef DRM_FORMAT_R8
    { amf::AMF_SURFACE_GRAY8,    DRM_FORMAT_R8       },
#endif
#ifdef DRM_FORMAT_R16
//    { , DRM_FORMAT_R16      },
//    { , DRM_FORMAT_R16      | DRM_FORMAT_BIG_ENDIAN },
#endif
//    { ,     DRM_FORMAT_BGR233   },
//    { , DRM_FORMAT_XRGB1555 },
//    { , DRM_FORMAT_XRGB1555 | DRM_FORMAT_BIG_ENDIAN },
//    { , DRM_FORMAT_XBGR1555 },
//    { , DRM_FORMAT_XBGR1555 | DRM_FORMAT_BIG_ENDIAN },
//    { , DRM_FORMAT_RGB565   },
//    { , DRM_FORMAT_RGB565   | DRM_FORMAT_BIG_ENDIAN },
//    { , DRM_FORMAT_BGR565   },
//    { , DRM_FORMAT_BGR565   | DRM_FORMAT_BIG_ENDIAN },
//    { ,    DRM_FORMAT_RGB888   },
//    { ,    DRM_FORMAT_BGR888   },
    { amf::AMF_SURFACE_BGRA,     DRM_FORMAT_BGRX8888 },
    { amf::AMF_SURFACE_RGBA,     DRM_FORMAT_RGBX8888 },
    { amf::AMF_SURFACE_BGRA,     DRM_FORMAT_XBGR8888 },
    { amf::AMF_SURFACE_BGRA /*AMF_SURFACE_ARGB*/,     DRM_FORMAT_XRGB8888 },
    { amf::AMF_SURFACE_RGBA,     DRM_FORMAT_BGRA8888 },
    { amf::AMF_SURFACE_ARGB,     DRM_FORMAT_ARGB8888 },
    { amf::AMF_SURFACE_YUY2,  DRM_FORMAT_YUYV     },
//    { ,  DRM_FORMAT_YVYU     },
    { amf::AMF_SURFACE_UYVY,  DRM_FORMAT_UYVY     },
};

amf::AMF_SURFACE_FORMAT AMF_STD_CALL FromDRMtoAMF(uint32_t formatDRM)
{
    for(int i = 0; i < amf_countof(formatMap); i++)
    {
        if(formatMap[i].formatDRM == formatDRM)
        {
            return formatMap[i].formatAMF;
        }
    }
    return amf::AMF_SURFACE_UNKNOWN;
}

drmModeFB2Ptr AMF_STD_CALL AMFdrmModeGetFB2(int fd, uint32_t fb_id)
{
	struct drm_mode_fb_cmd2 get = {
		.fb_id = fb_id,
	};
	drmModeFB2Ptr ret;
	int err;

	err = drmIoctl(fd, DRM_IOCTL_MODE_GETFB2, &get);
	if (err != 0)
		return NULL;

	ret = (drmModeFB2Ptr)drmMalloc(sizeof(drmModeFB2));
	if (!ret)
		return NULL;

	ret->fb_id = fb_id;
	ret->width = get.width;
	ret->height = get.height;
	ret->pixel_format = get.pixel_format;
	ret->flags = get.flags;
	ret->modifier = get.modifier[0];
	memcpy(ret->handles, get.handles, sizeof(uint32_t) * 4);
	memcpy(ret->pitches, get.pitches, sizeof(uint32_t) * 4);
	memcpy(ret->offsets, get.offsets, sizeof(uint32_t) * 4);

	return ret;
}

void AMF_STD_CALL AMFdrmModeFreeFB2(drmModeFB2Ptr ptr)
{
	drmFree(ptr);
}

DRMDevice::DRMDevice() {}

DRMDevice::~DRMDevice()
{
	Terminate();
}

AMF_RESULT AMF_STD_CALL DRMDevice::InitFromVulkan(int pciDomain, int pciBus, int pciDevice, int pciFunction)
{
    int dirfd = open("/dev/dri/by-path", O_RDONLY);
    AMF_RETURN_IF_FALSE(dirfd != -1, AMF_FAIL, L"Couldn't open /dev/dri/by-path")
    DIR *pDir = fdopendir(dirfd);
    if (pDir == nullptr)
    {   
        close(dirfd);
        return AMF_FAIL;
    }

    struct dirent *entry;
    while ((entry = readdir(pDir)) != NULL)
    {
        int entryDomain = -1, entryBus = -1, entryDevice = -1, entryFunction = -1, length = -1;

        int res = sscanf(entry->d_name, "pci-%x:%x:%x.%x-card%n",
            &entryDomain, &entryBus, &entryDevice, &entryFunction, &length);
        //check if matches pattern
        if (res != 4 || length != strlen(entry->d_name))
        {
            continue;
        }
        if (entryDomain == pciDomain && entryBus == pciBus && entryDevice == pciDevice && entryFunction == pciFunction)
        {
            m_fd = openat(dirfd, entry->d_name, O_RDWR | O_CLOEXEC);
            m_pathToCard = entry->d_name;
            break;
        }
    }

    closedir(pDir); //implicitly closes dirfd
    if (m_fd < 0)
    {
    	return AMF_FAIL;
    }
    return SetupDevice();
}

AMF_RESULT AMF_STD_CALL DRMDevice::InitFromPath(const char* pathToCard)
{
	m_fd = open(pathToCard, O_RDWR | O_CLOEXEC);
	m_pathToCard = pathToCard;

    if (m_fd < 0)
    {
    	return AMF_FAIL;
    }
    return SetupDevice();
}

AMF_RESULT DRMDevice::SetupDevice()
{
    drmVersionPtr version = drmGetVersion(m_fd);
    AMF_RETURN_IF_FALSE(version != nullptr, AMF_FAIL, L"drmGetVersion() failed from %S", m_pathToCard.c_str());

    AMFTraceDebug(AMF_FACILITY, L"Opened DRM device %S: driver name %S version %d.%d.%d", m_pathToCard.c_str(), version->name,
            version->version_major, version->version_minor, version->version_patchlevel);

    drmFreeVersion(version);

    uint64_t valueExport = 0;
    int err = drmGetCap(m_fd, DRM_PRIME_CAP_EXPORT, &valueExport);

    err = drmSetClientCap(m_fd, DRM_CLIENT_CAP_UNIVERSAL_PLANES, 1);
    if (err < 0)
    {
        AMFTraceWarning(AMF_FACILITY, L"drmSetClientCap(DRM_CLIENT_CAP_UNIVERSAL_PLANES) Failed with %d", err);
    }
    drmSetClientCap(m_fd, DRM_CLIENT_CAP_ATOMIC, 1);

    return AMF_OK;
}

AMF_RESULT AMF_STD_CALL DRMDevice::Terminate()
{
	if (m_fd >= 0)
	{
		close(m_fd);
		m_fd = -1;
	}
	m_pathToCard = "";
	return AMF_OK;
}

int AMF_STD_CALL DRMDevice::GetFD() const
{
	return m_fd;
}

std::string AMF_STD_CALL DRMDevice::GetPathToCard() const
{
	return m_pathToCard;
}

AMF_RESULT AMF_STD_CALL DRMDevice::GetCRTCs(std::vector<DRMCRTC>& crtcs) const
{
    AMF_RETURN_IF_FALSE(m_fd >= 0, AMF_FAIL, L"Not Initialized");

    AMFdrmModeResPtr resources = drmModeGetResources(m_fd);
    AMF_RETURN_IF_FALSE(resources.p != nullptr, AMF_FAIL, L"drmModeGetResources() return nullptr");

    crtcs.clear();
    for(int i = 0; i < resources.p->count_crtcs; i ++)
    {
        AMFdrmModeCrtcPtr crtc = drmModeGetCrtc(m_fd, resources.p->crtcs[i]);

	    AMFRect crop = {};
	    amf::AMF_SURFACE_FORMAT formatAMF = amf::AMF_SURFACE_UNKNOWN;
	    int formatDRM = 0;
	    int handle = 0;
        if(GetCrtcInfo(crtc, crop, formatDRM, formatAMF, handle) != AMF_OK)
        {
            continue;
        }
        AMFTraceDebug(AMF_FACILITY, L"    CRTC id=%d fb=%d crop(%d,%d,%d,%d)", crtc.p->crtc_id, crtc.p->buffer_id,  crop.left, crop.top, crop.right, crop.bottom);

        DRMCRTC drmCrtc = {};
        drmCrtc.crtcID = crtc.p->crtc_id;
        drmCrtc.fbID = crtc.p->buffer_id;
        drmCrtc.crop = crop;
        drmCrtc.formatDRM = formatDRM;
        drmCrtc.formatAMF = formatAMF;
        drmCrtc.handle = handle;
        crtcs.push_back(drmCrtc);
    }
    return AMF_OK;
}


AMF_RESULT AMF_STD_CALL DRMDevice::GetCrtcInfo(const AMFdrmModeCrtcPtr& crtc, AMFRect &crop, int& formatDRM, amf::AMF_SURFACE_FORMAT& formatAMF, int& handle) const
{
    if(crtc.p == nullptr)
    {
        return AMF_FAIL;
    }
    if(crtc.p->buffer_id == 0)
    {
        return AMF_FAIL;
    }
    // check if active
    AMFdrmModeObjectPropertiesPtr properties = drmModeObjectGetProperties 	(m_fd, crtc.p->crtc_id, DRM_MODE_OBJECT_CRTC);
    if(properties.p == nullptr)
    {
        return AMF_FAIL;
    }

    for(int k = 0; k < properties.p->count_props; k++)
    {
        AMFdrmModePropertyPtr prop = drmModeGetProperty(m_fd, properties.p->props[k]);

        if(std::string(prop.p->name) == "ACTIVE" && properties.p->prop_values[k] == 0)
        {
            return AMF_FAIL;
        }
    }
    // check FB

    AMFdrmModeFB2Ptr fb2 = AMFdrmModeGetFB2(m_fd, crtc.p->buffer_id);
    if(fb2.p == nullptr)
    {
        return AMF_FAIL;
    }

    crop.left = crtc.p->x;
    crop.top = crtc.p->y;
    crop.right = crtc.p->x + crtc.p->width;
    crop.bottom = crtc.p->y + crtc.p->height;
    formatDRM = fb2.p->pixel_format;
    formatAMF= FromDRMtoAMF(fb2.p->pixel_format);
    handle = fb2.p->handles[0];

    return AMF_OK;
}
