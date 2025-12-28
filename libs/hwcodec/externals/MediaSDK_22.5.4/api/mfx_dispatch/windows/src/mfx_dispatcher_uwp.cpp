// Copyright (c) 2020 Intel Corporation
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#include "mfx_dispatcher.h"
#include "mfx_dispatcher_uwp.h"
#include "mfx_driver_store_loader.h"
#include "mfx_dxva2_device.h"
#include "mfx_load_dll.h"

mfxStatus GfxApiInit(mfxInitParam par, mfxU32 deviceID, mfxSession *session, mfxModuleHandle& hModule)
{
    HRESULT hr = S_OK;

    if (hModule == NULL)
    {
        wchar_t IntelGFXAPIdllName[MFX_MAX_DLL_PATH] = { 0 };
        MFX::DriverStoreLoader dsLoader;

        if (!dsLoader.GetDriverStorePath(IntelGFXAPIdllName, sizeof(IntelGFXAPIdllName), deviceID))
        {
            return MFX_ERR_UNSUPPORTED;
        }

        size_t pathLen = wcslen(IntelGFXAPIdllName);
        MFX::mfx_get_default_intel_gfx_api_dll_name(IntelGFXAPIdllName + pathLen, sizeof(IntelGFXAPIdllName) / sizeof(IntelGFXAPIdllName[0]) - pathLen);
        DISPATCHER_LOG_INFO((("loading %S\n"), IntelGFXAPIdllName));

        hModule = MFX::mfx_dll_load(IntelGFXAPIdllName);
        if (!hModule)
        {
            DISPATCHER_LOG_ERROR("Can't load intel_gfx_api\n");
            return MFX_ERR_UNSUPPORTED;
        }
    }

    mfxFunctionPointer pFunc = (mfxFunctionPointer)MFX::mfx_dll_get_addr(hModule, "InitialiseMediaSession");
    if (!pFunc)
    {
        DISPATCHER_LOG_ERROR("Can't find required API function: InitialiseMediaSession\n");
        MFX::mfx_dll_free(hModule);
        return MFX_ERR_UNSUPPORTED;
    }

    typedef HRESULT(APIENTRY *InitialiseMediaSessionPtr) (HANDLE*, LPVOID, LPVOID);
    InitialiseMediaSessionPtr init = (InitialiseMediaSessionPtr)pFunc;
    hr = init((HANDLE*)session, &par, NULL);

    return (hr == S_OK) ? MFX_ERR_NONE : MFX_ERR_UNKNOWN;
}

mfxStatus GfxApiClose(mfxSession& session, mfxModuleHandle& hModule)
{
    HRESULT hr = S_OK;

    if (!hModule)
    {
        return MFX_ERR_NULL_PTR;
    }

    mfxFunctionPointer pFunc = (mfxFunctionPointer)MFX::mfx_dll_get_addr(hModule, "DisposeMediaSession");
    if (!pFunc)
    {
        DISPATCHER_LOG_ERROR("Can't find required API function: DisposeMediaSession\n");
        return MFX_ERR_INVALID_HANDLE;
    }

    typedef HRESULT(APIENTRY *DisposeMediaSessionPtr) (HANDLE);
    DisposeMediaSessionPtr dispose = (DisposeMediaSessionPtr)pFunc;
    hr = dispose((HANDLE)session);
    session = NULL;

    MFX::mfx_dll_free(hModule);
    hModule = NULL;

    return (hr == S_OK) ? MFX_ERR_NONE : MFX_ERR_UNKNOWN;
}

mfxStatus GfxApiInitByAdapterNum(mfxInitParam par, mfxU32 adapterNum, mfxSession *session, mfxModuleHandle& hModule)
{
    MFX::DXVA2Device dxvaDevice;

    if (!dxvaDevice.InitDXGI1(adapterNum))
    {
        DISPATCHER_LOG_ERROR((("dxvaDevice.InitDXGI1(%d) Failed\n"), adapterNum));
        return MFX_ERR_UNSUPPORTED;
    }

    if (dxvaDevice.GetVendorID() != INTEL_VENDOR_ID)
    {
        DISPATCHER_LOG_ERROR("Specified adapter is not Intel\n");
        return MFX_ERR_UNSUPPORTED;
    }

    return GfxApiInit(par, dxvaDevice.GetDeviceID(), session, hModule);
}

struct GfxApiHandle
{
    mfxModuleHandle hModule;
    mfxSession session;
    mfxU16 mediaAdapterType;
};

static int GfxApiHandleSort(const void * plhs, const void * prhs)
{
    const GfxApiHandle * lhs = *(const GfxApiHandle **)plhs;
    const GfxApiHandle * rhs = *(const GfxApiHandle **)prhs;

    // prefer integrated GPU
    if (lhs->mediaAdapterType != MFX_MEDIA_INTEGRATED && rhs->mediaAdapterType == MFX_MEDIA_INTEGRATED)
    {
        return 1;
    }
    if (lhs->mediaAdapterType == MFX_MEDIA_INTEGRATED && rhs->mediaAdapterType != MFX_MEDIA_INTEGRATED)
    {
        return -1;
    }

    return 0;
}

mfxStatus GfxApiInitPriorityIntegrated(mfxInitParam par, mfxSession *session, mfxModuleHandle& hModule)
{
    mfxStatus sts = MFX_ERR_UNSUPPORTED;
    MFX::MFXVector<GfxApiHandle> gfxApiHandles;

    for (int adapterNum = 0; adapterNum < 4; ++adapterNum)
    {
        MFX::DXVA2Device dxvaDevice;

        if (!dxvaDevice.InitDXGI1(adapterNum) || dxvaDevice.GetVendorID() != INTEL_VENDOR_ID)
        {
            continue;
        }

        par.Implementation &= ~(0xf);
        switch (adapterNum)
        {
        case 0:
            par.Implementation |= MFX_IMPL_HARDWARE;
            break;
        case 1:
            par.Implementation |= MFX_IMPL_HARDWARE2;
            break;
        case 2:
            par.Implementation |= MFX_IMPL_HARDWARE3;
            break;
        case 3:
            par.Implementation |= MFX_IMPL_HARDWARE4;
            break;
        }

        mfxModuleHandle hModuleCur = NULL;
        mfxSession sessionCur = NULL;

        sts = GfxApiInit(par, dxvaDevice.GetDeviceID(), &sessionCur, hModuleCur);
        if (sts != MFX_ERR_NONE)
            continue;

        mfxPlatform platform = { MFX_PLATFORM_UNKNOWN, 0, MFX_MEDIA_UNKNOWN };
        sts = MFXVideoCORE_QueryPlatform(sessionCur, &platform);
        if (sts != MFX_ERR_NONE)
        {
            sts = GfxApiClose(sessionCur, hModuleCur);
            if (sts != MFX_ERR_NONE)
                return sts;
            continue;
        }

        GfxApiHandle handle = { hModuleCur, sessionCur, platform.MediaAdapterType };
        gfxApiHandles.push_back(handle);
    }

    //Try to use fallback from System folder
    mfxModuleHandle hFallback = NULL;

    if (gfxApiHandles.size() == 0)
    {
        wchar_t IntelGFXAPIdllName[MFX_MAX_DLL_PATH] = { 0 };
        MFX::mfx_get_default_intel_gfx_api_dll_name(IntelGFXAPIdllName, sizeof(IntelGFXAPIdllName) / sizeof(IntelGFXAPIdllName[0]));
        DISPATCHER_LOG_INFO((("loading fallback %S\n"), IntelGFXAPIdllName));

        hFallback = MFX::mfx_dll_load(IntelGFXAPIdllName);
        if (!hFallback)
        {
            DISPATCHER_LOG_ERROR("Can't load intel_gfx_api\n");
            return MFX_ERR_UNSUPPORTED;
        }

        for (int adapterNum = 0; adapterNum < 4; ++adapterNum)
        {
            MFX::DXVA2Device dxvaDevice;

            if (!dxvaDevice.InitDXGI1(adapterNum) || dxvaDevice.GetVendorID() != INTEL_VENDOR_ID)
            {
                continue;
            }

            par.Implementation &= ~(0xf);
            switch (adapterNum)
            {
            case 0:
                par.Implementation |= MFX_IMPL_HARDWARE;
                break;
            case 1:
                par.Implementation |= MFX_IMPL_HARDWARE2;
                break;
            case 2:
                par.Implementation |= MFX_IMPL_HARDWARE3;
                break;
            case 3:
                par.Implementation |= MFX_IMPL_HARDWARE4;
                break;
            }

            mfxSession sessionCur = NULL;

            sts = GfxApiInit(par, dxvaDevice.GetDeviceID(), &sessionCur, hFallback);
            if (sts != MFX_ERR_NONE)
                continue;

            mfxPlatform platform = { MFX_PLATFORM_UNKNOWN, 0, MFX_MEDIA_UNKNOWN };
            sts = MFXVideoCORE_QueryPlatform(sessionCur, &platform);
            if (sts != MFX_ERR_NONE)
            {
                continue;
            }

            GfxApiHandle handle = { NULL, sessionCur, platform.MediaAdapterType };
            gfxApiHandles.push_back(handle);
        }
    }

    if (gfxApiHandles.size() == 0)
    {
        if (hFallback != NULL)
        {
            MFX::mfx_dll_free(hFallback);
            hFallback = NULL;
        }

        return MFX_ERR_UNSUPPORTED;
    }

    qsort(&(*gfxApiHandles.begin()), gfxApiHandles.size(), sizeof(GfxApiHandle), &GfxApiHandleSort);

    // When hModule == NULL and hFallback != NULL - it means dispatcher uses fallback library from System folder
    hModule = ( gfxApiHandles.begin()->hModule == NULL && hFallback != NULL )? hFallback : gfxApiHandles.begin()->hModule;
    *session = gfxApiHandles.begin()->session;

    MFX::MFXVector<GfxApiHandle>::iterator it = gfxApiHandles.begin()++;
    for (; it != gfxApiHandles.end(); ++it)
    {
        sts = GfxApiClose(it->session, it->hModule);

        if (sts == MFX_ERR_NULL_PTR)
            continue;

        if (sts != MFX_ERR_NONE)
            return sts;
    }

    //If dispatcher has tried a fallback, but returns something else - free loaded fallback
    if (hFallback != NULL && hModule != hFallback)
    {
        MFX::mfx_dll_free(hFallback);
        hFallback = NULL;
    }

    return sts;
}
