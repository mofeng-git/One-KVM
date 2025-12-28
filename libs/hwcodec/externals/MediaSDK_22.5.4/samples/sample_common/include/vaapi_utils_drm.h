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

#ifndef __VAAPI_UTILS_DRM_H__
#define __VAAPI_UTILS_DRM_H__

#if defined(LIBVA_DRM_SUPPORT)

#include <va/va_drm.h>
#include <va/va_drmcommon.h>
#include "vaapi_utils.h"
#include "vaapi_allocator.h"

class drmRenderer;

class DRMLibVA : public CLibVA
{
public:
    DRMLibVA(const std::string& devicePath = "", int type = MFX_LIBVA_DRM);
    virtual ~DRMLibVA(void);

    inline int getFD() { return m_fd; }

protected:
    int m_fd;
    MfxLoader::VA_DRMProxy m_vadrmlib;

private:
    DISALLOW_COPY_AND_ASSIGN(DRMLibVA);
};

class drmRenderer : public vaapiAllocatorParams::Exporter
{
public:
    drmRenderer(int fd, mfxI32 monitorType);
    virtual ~drmRenderer();

    virtual mfxStatus render(mfxFrameSurface1 * pSurface);

    // vaapiAllocatorParams::Exporter methods
    virtual void* acquire(mfxMemId mid);
    virtual void release(mfxMemId mid, void * mem);

    static uint32_t getConnectorType(mfxI32 monitor_type);
    static const msdk_char* getConnectorName(uint32_t connector_type);

private:
    bool getConnector(drmModeRes *resource, uint32_t connector_type);
    bool setupConnection(drmModeRes *resource, drmModeConnector* connector);
    bool getPlane();

    bool setMaster();
    void dropMaster();
    bool restore();

    const MfxLoader::DRM_Proxy m_drmlib;
    const MfxLoader::DrmIntel_Proxy m_drmintellib;

    int m_fd;
    uint32_t m_connector_type;
    uint32_t m_connectorID;
    uint32_t m_encoderID;
    uint32_t m_crtcID;
    uint32_t m_crtcIndex;
    uint32_t m_planeID;
    drmModeModeInfo m_mode;
    drmModeCrtcPtr m_crtc;
    drm_intel_bufmgr* m_bufmgr;
    bool m_overlay_wrn;
    mfxFrameSurface1 * m_pCurrentRenderTargetSurface;

private:
    DISALLOW_COPY_AND_ASSIGN(drmRenderer);
};

#endif // #if defined(LIBVA_DRM_SUPPORT)

#endif // #ifndef __VAAPI_UTILS_DRM_H__
