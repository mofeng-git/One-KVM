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


#ifndef __DECODE_D3D_RENDER_H__
#define __DECODE_D3D_RENDER_H__

#if defined(_WIN32) || defined(_WIN64)

#pragma warning(disable : 4201)
#include <d3d9.h>
#include <dxva2api.h>
#include <dxva.h>
#include <windows.h>
#endif

#include "mfxstructures.h"
#include "mfxvideo.h"

#include "hw_device.h"

typedef void* WindowHandle;
typedef void* Handle;

#if defined(_WIN32) || defined(_WIN64)

struct sWindowParams
{
    LPCTSTR lpClassName;
    LPCTSTR lpWindowName;
    DWORD dwStyle;
    int nx;
    int ny;
    int ncell;
    int nAdapter;
    int nWidth;
    int nHeight;
    HWND hWndParent;
    HMENU hMenu;
    HINSTANCE hInstance;
    LPVOID lpParam;
    bool bFullScreen; // Stretch window to full screen
};

class CDecodeD3DRender
{
public:

    CDecodeD3DRender();
    virtual ~CDecodeD3DRender();

    virtual mfxStatus Init(sWindowParams pWParams);
    virtual mfxStatus RenderFrame(mfxFrameSurface1 *pSurface, mfxFrameAllocator *pmfxAlloc);
    virtual VOID UpdateTitle(double fps);
    void Close();

    HWND GetWindowHandle();

    VOID OnDestroy(HWND hwnd);
    VOID OnKey(HWND hwnd, UINT vk, BOOL fDown, int cRepeat, UINT flags);
    VOID ChangeWindowSize(bool bFullScreen);

    void SetHWDevice(CHWDevice *dev)
    {
        m_hwdev = dev;
    }
protected:
    void AdjustWindowRect(RECT *rect);

    mfxStatus AllocateShiftedSurfaceIfNeeded(const mfxFrameSurface1* refSurface,mfxFrameAllocator* allocator);
    mfxFrameAllocResponse shiftSurfaceResponse;
    mfxFrameSurface1 shiftedSurface;
    mfxFrameAllocator* pAllocator;

    CHWDevice *m_hwdev;


    sWindowParams       m_sWindowParams;
    HWND                m_Hwnd;
    RECT                m_rect;
    DWORD               m_style;

    bool EnableDwmQueuing();
    static BOOL CALLBACK MonitorEnumProc(HMONITOR ,HDC ,LPRECT lprcMonitor,LPARAM dwData);
    static bool m_bIsMonitorFound;

    bool                 m_bDwmEnabled;
    int                  m_nMonitorCurrent;
    ::RECT               m_RectWindow;
};
#endif // #if defined(_WIN32) || defined(_WIN64)

#endif // __DECODE_D3D_RENDER_H__