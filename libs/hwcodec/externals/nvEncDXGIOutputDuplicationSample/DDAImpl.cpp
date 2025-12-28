/*
 * Copyright (c) 2019, NVIDIA CORPORATION. All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *  * Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 *  * Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *  * Neither the name of NVIDIA CORPORATION nor the names of its
 *    contributors may be used to endorse or promote products derived
 *    from this software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS ``AS IS'' AND ANY
 * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
 * PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT OWNER OR
 * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
 * EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
 * PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
 * PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY
 * OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

#include "Defs.h"
#include "DDAImpl.h"
#include <iomanip>

/// Initialize DDA
HRESULT DDAImpl::Init()
{
    IDXGIOutput * pOutput = nullptr;
    IDXGIDevice2* pDevice = nullptr;
    IDXGIFactory1* pFactory = nullptr;
    IDXGIAdapter *pAdapter = nullptr;
    IDXGIOutput1* pOut1 = nullptr;

    /// Release all temporary refs before exit
#define CLEAN_RETURN(x) \
    SAFE_RELEASE(pDevice);\
    SAFE_RELEASE(pFactory);\
    SAFE_RELEASE(pOutput);\
    SAFE_RELEASE(pOut1);\
    SAFE_RELEASE(pAdapter);\
    return x;

    HRESULT hr = S_OK;
    /// To create a DDA object given a D3D11 device, we must first get to the DXGI Adapter associated with that device
    if (FAILED(hr = pD3DDev->QueryInterface(__uuidof(IDXGIDevice2), (void**)&pDevice)))
    {
        CLEAN_RETURN(hr);
    }

    if (FAILED(hr = pDevice->GetParent(__uuidof(IDXGIAdapter), (void**)&pAdapter)))
    {
        CLEAN_RETURN(hr);
    }
    /// Once we have the DXGI Adapter, we enumerate the attached display outputs, and select which one we want to capture
    /// This sample application always captures the primary display output, enumerated at index 0.
    if (FAILED(hr = pAdapter->EnumOutputs(0, &pOutput)))
    {
        CLEAN_RETURN(hr);
    }

    if (FAILED(hr = pOutput->QueryInterface(__uuidof(IDXGIOutput1), (void**)&pOut1)))
    {
        CLEAN_RETURN(hr);
    }
    /// Ask DXGI to create an instance of IDXGIOutputDuplication for the selected output. We can now capture this display output
    if (FAILED(hr = pOut1->DuplicateOutput(pDevice, &pDup)))
    {
        CLEAN_RETURN(hr);
    }

    DXGI_OUTDUPL_DESC outDesc;
    ZeroMemory(&outDesc, sizeof(outDesc));
    pDup->GetDesc(&outDesc);

    height = outDesc.ModeDesc.Height;
    width = outDesc.ModeDesc.Width;
    CLEAN_RETURN(hr);
}

/// Acquire a new frame from DDA, and return it as a Texture2D object.
/// 'wait' specifies the time in milliseconds that DDA shoulo wait for a new screen update.
HRESULT DDAImpl::GetCapturedFrame(ID3D11Texture2D **ppTex2D, int wait, bool fail_if_equal)
{
    HRESULT hr = S_OK;
    DXGI_OUTDUPL_FRAME_INFO frameInfo;
    ZeroMemory(&frameInfo, sizeof(frameInfo));
    int acquired = 0;
    

#define RETURN_ERR(x) {printf(__FUNCTION__": %d : Line %d return 0x%x\n", frameno, __LINE__, x);return x;}

    if (pResource)
    {
        pDup->ReleaseFrame();
        pResource->Release();
        pResource = nullptr;
    }

    hr = pDup->AcquireNextFrame(wait, &frameInfo, &pResource);
    if (FAILED(hr))
    {
        if (hr == DXGI_ERROR_WAIT_TIMEOUT)
        {
            // printf(__FUNCTION__": %d : Wait for %d ms timed out\n", frameno, wait);
        }
        if (hr == DXGI_ERROR_INVALID_CALL)
        {
            printf(__FUNCTION__": %d : Invalid Call, previous frame not released?\n", frameno);
        }
        if (hr == DXGI_ERROR_ACCESS_LOST)
        {
            printf(__FUNCTION__": %d : Access lost, frame needs to be released?\n", frameno);
        }
        // RETURN_ERR(hr);
        return hr;
    }
    if (fail_if_equal)
    {
        if (frameInfo.AccumulatedFrames == 0 || frameInfo.LastPresentTime.QuadPart == 0)
        {
            // No image update, only cursor moved.
            RETURN_ERR(DXGI_ERROR_WAIT_TIMEOUT);
        }
    }

    if (!pResource)
    {
        printf(__FUNCTION__": %d : Null output resource. Return error.\n", frameno);
        return E_UNEXPECTED;
    }

    if (FAILED(hr = pResource->QueryInterface(__uuidof(ID3D11Texture2D), (void**)ppTex2D)))
    {
        return hr;
    }

    LARGE_INTEGER pts = frameInfo.LastPresentTime;  MICROSEC_TIME(pts, qpcFreq);
    LONGLONG interval = pts.QuadPart - lastPTS.QuadPart;

    // printf(__FUNCTION__": %d : Accumulated Frames %u PTS Interval %lld PTS %lld\n", frameno, frameInfo.AccumulatedFrames,  interval * 1000, frameInfo.LastPresentTime.QuadPart);
    lastPTS = pts; // store microsec value
    frameno += frameInfo.AccumulatedFrames;
    return hr;
}

/// Release all resources
int DDAImpl::Cleanup()
{
    if (pResource)
    {
        pDup->ReleaseFrame();
        SAFE_RELEASE(pResource);
    }

    width = height = frameno = 0;

    SAFE_RELEASE(pDup);
    SAFE_RELEASE(pCtx);
    SAFE_RELEASE(pD3DDev);

    return 0;
}