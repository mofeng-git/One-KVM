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

#ifndef __VAAPI_UTILS_X11_H__
#define __VAAPI_UTILS_X11_H__

#if defined(LIBVA_X11_SUPPORT)

#include <va/va_x11.h>
#include "vaapi_utils.h"

class X11LibVA : public CLibVA
{
public:
    X11LibVA(void);
    virtual ~X11LibVA(void);

    void *GetXDisplay(void) { return m_display;}


    MfxLoader::XLib_Proxy  & GetX11() { return m_x11lib; }
    MfxLoader::VA_X11Proxy & GetVAX11() { return m_vax11lib; }
#if defined(X11_DRI3_SUPPORT)
    MfxLoader::Xcb_Proxy & GetXcbX11() { return m_xcblib; }
    MfxLoader::X11_Xcb_Proxy & GetX11XcbX11() { return m_x11xcblib; }
    MfxLoader::XCB_Dri3_Proxy   & GetXCBDri3X11() { return m_xcbdri3lib; }
    MfxLoader::Xcbpresent_Proxy & GetXcbpresentX11() { return m_xcbpresentlib; }
    MfxLoader::DrmIntel_Proxy & GetDrmIntelX11() { return m_drmintellib; }
#endif // X11_DRI3_SUPPORT

protected:
    Display* m_display;
    VAConfigID  m_configID;
    VAContextID m_contextID;
    MfxLoader::XLib_Proxy   m_x11lib;
    MfxLoader::VA_X11Proxy  m_vax11lib;
#if defined(X11_DRI3_SUPPORT)
    MfxLoader::VA_DRMProxy  m_vadrmlib;
    MfxLoader::Xcb_Proxy    m_xcblib;
    MfxLoader::X11_Xcb_Proxy    m_x11xcblib;
    MfxLoader::XCB_Dri3_Proxy   m_xcbdri3lib;
    MfxLoader::Xcbpresent_Proxy m_xcbpresentlib;
    MfxLoader::DrmIntel_Proxy   m_drmintellib;
#endif // X11_DRI3_SUPPORT

private:
    void Close();

    DISALLOW_COPY_AND_ASSIGN(X11LibVA);
};

#endif // #if defined(LIBVA_X11_SUPPORT)

#endif // #ifndef __VAAPI_UTILS_X11_H__
