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

#ifndef __VAAPI_ALLOCATOR_H__
#define __VAAPI_ALLOCATOR_H__

#if defined(LIBVA_SUPPORT)

#include <stdlib.h>
#include <va/va.h>
#include <va/va_drmcommon.h>

#include "base_allocator.h"
#include "vaapi_utils.h"

// VAAPI Allocator internal Mem ID
struct vaapiMemId
{
    VASurfaceID* m_surface;
    VAImage      m_image;
    // variables for VAAPI Allocator internal color conversion
    unsigned int m_fourcc;
    mfxU8*       m_sys_buffer;
    mfxU8*       m_va_buffer;
    // buffer info to support surface export
    VABufferInfo m_buffer_info;
    // pointer to private export data
    void*        m_custom;
};

namespace MfxLoader
{
    class VA_Proxy;
}

struct vaapiAllocatorParams : mfxAllocatorParams
{
    enum {
      DONOT_EXPORT = 0,
      FLINK = 0x01,
      PRIME = 0x02,
      NATIVE_EXPORT_MASK = FLINK | PRIME,
      CUSTOM = 0x100,
      CUSTOM_FLINK = CUSTOM | FLINK,
      CUSTOM_PRIME = CUSTOM | PRIME
    };
    class Exporter
    {
    public:
      virtual ~Exporter(){}
      virtual void* acquire(mfxMemId mid) = 0;
      virtual void release(mfxMemId mid, void * hdl) = 0;
    };

    vaapiAllocatorParams()
      : m_dpy(NULL)
      , m_export_mode(DONOT_EXPORT)
      , m_exporter(NULL)
    {}

    VADisplay m_dpy;
    mfxU32 m_export_mode;
    Exporter* m_exporter;
};

class vaapiFrameAllocator: public BaseFrameAllocator
{
public:
    vaapiFrameAllocator();
    virtual ~vaapiFrameAllocator();

    virtual mfxStatus Init(mfxAllocatorParams *pParams);
    virtual mfxStatus Close();

protected:
    DISALLOW_COPY_AND_ASSIGN(vaapiFrameAllocator);

    virtual mfxStatus LockFrame(mfxMemId mid, mfxFrameData *ptr);
    virtual mfxStatus UnlockFrame(mfxMemId mid, mfxFrameData *ptr);
    virtual mfxStatus GetFrameHDL(mfxMemId mid, mfxHDL *handle);

    virtual mfxStatus CheckRequestType(mfxFrameAllocRequest *request);
    virtual mfxStatus ReleaseResponse(mfxFrameAllocResponse *response);
    virtual mfxStatus AllocImpl(mfxFrameAllocRequest *request, mfxFrameAllocResponse *response);
    virtual mfxStatus ReallocImpl(mfxMemId midIn, const mfxFrameInfo *info, mfxU16 memType, mfxMemId *midOut);

    VADisplay m_dpy;
    MfxLoader::VA_Proxy * m_libva;
    mfxU32 m_export_mode;
    vaapiAllocatorParams::Exporter* m_exporter;
};

#endif //#if defined(LIBVA_SUPPORT)

#endif // __VAAPI_ALLOCATOR_H__
