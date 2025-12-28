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

#ifdef LIBVA_ANDROID_SUPPORT
#ifdef ANDROID

#include "vaapi_utils_android.h"

CLibVA* CreateLibVA(int)
{
    return new AndroidLibVA;
}

/*------------------------------------------------------------------------------*/

typedef unsigned int vaapiAndroidDisplay;

#define VAAPI_ANDROID_DEFAULT_DISPLAY 0x18c34078

AndroidLibVA::AndroidLibVA(void)
    : CLibVA(MFX_LIBVA_AUTO)
    , m_display(NULL)
{
    VAStatus va_res = VA_STATUS_SUCCESS;
    mfxStatus sts = MFX_ERR_NONE;
    int major_version = 0, minor_version = 0;
    vaapiAndroidDisplay* display = NULL;

    m_display = display = (vaapiAndroidDisplay*)malloc(sizeof(vaapiAndroidDisplay));
    if (NULL == m_display) sts = MFX_ERR_NOT_INITIALIZED;
    else *display = VAAPI_ANDROID_DEFAULT_DISPLAY;

    if (MFX_ERR_NONE == sts)
    {
        m_va_dpy = vaGetDisplay(m_display);
        if (!m_va_dpy)
        {
            free(m_display);
            sts = MFX_ERR_NULL_PTR;
        }
    }
    if (MFX_ERR_NONE == sts)
    {
        va_res = vaInitialize(m_va_dpy, &major_version, &minor_version);
        sts = va_to_mfx_status(va_res);
        if (MFX_ERR_NONE != sts)
        {
            free(display);
            m_display = NULL;
        }
    }
    if (MFX_ERR_NONE != sts) throw std::bad_alloc();
}

AndroidLibVA::~AndroidLibVA(void)
{
    if (m_va_dpy)
    {
        vaTerminate(m_va_dpy);
    }
    if (m_display)
    {
        free(m_display);
    }
}

#endif // #ifdef ANDROID
#endif // #ifdef LIBVA_ANDROID_SUPPORT
