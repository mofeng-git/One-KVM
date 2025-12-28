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

#ifdef LIBVA_SUPPORT

#include "vaapi_utils.h"
#include <dlfcn.h>
#include <stdexcept>

//#if defined(LIBVA_DRM_SUPPORT)
#include "vaapi_utils_drm.h"
//#elif defined(LIBVA_X11_SUPPORT)
#include "vaapi_utils_x11.h"
//#endif

#if defined(LIBVA_WAYLAND_SUPPORT)
#include "class_wayland.h"
#endif

namespace MfxLoader
{

SimpleLoader::SimpleLoader(const char * name)
{
    dlerror();
    so_handle = dlopen(name, RTLD_GLOBAL | RTLD_NOW);
    if (NULL == so_handle)
    {
        std::cerr << dlerror() << std::endl;
        throw std::runtime_error("Can't load library");
    }
}

void * SimpleLoader::GetFunction(const char * name)
{
    void * fn_ptr = dlsym(so_handle, name);
    if (!fn_ptr)
        throw std::runtime_error("Can't find function");
    return fn_ptr;
}

SimpleLoader::~SimpleLoader()
{
    dlclose(so_handle);
}

#define SIMPLE_LOADER_STRINGIFY1( x) #x
#define SIMPLE_LOADER_STRINGIFY(x) SIMPLE_LOADER_STRINGIFY1(x)
#define SIMPLE_LOADER_DECORATOR1(fun,suffix) fun ## _ ## suffix
#define SIMPLE_LOADER_DECORATOR(fun,suffix) SIMPLE_LOADER_DECORATOR1(fun,suffix)


// Following macro applied on vaInitialize will give:  vaInitialize((vaInitialize_type)lib.GetFunction("vaInitialize"))
#define SIMPLE_LOADER_FUNCTION(name) name( (SIMPLE_LOADER_DECORATOR(name, type)) lib.GetFunction(SIMPLE_LOADER_STRINGIFY(name)) )


#if defined(LIBVA_SUPPORT)
VA_Proxy::VA_Proxy()
#ifdef ANDROID
    : lib("libva-android.so")
#else
    : lib("libva.so.2")
#endif
    , SIMPLE_LOADER_FUNCTION(vaInitialize)
    , SIMPLE_LOADER_FUNCTION(vaTerminate)
    , SIMPLE_LOADER_FUNCTION(vaCreateSurfaces)
    , SIMPLE_LOADER_FUNCTION(vaDestroySurfaces)
    , SIMPLE_LOADER_FUNCTION(vaCreateBuffer)
    , SIMPLE_LOADER_FUNCTION(vaDestroyBuffer)
    , SIMPLE_LOADER_FUNCTION(vaMapBuffer)
    , SIMPLE_LOADER_FUNCTION(vaUnmapBuffer)
    , SIMPLE_LOADER_FUNCTION(vaSyncSurface)
    , SIMPLE_LOADER_FUNCTION(vaDeriveImage)
    , SIMPLE_LOADER_FUNCTION(vaDestroyImage)
    , SIMPLE_LOADER_FUNCTION(vaGetLibFunc)
    , SIMPLE_LOADER_FUNCTION(vaAcquireBufferHandle)
    , SIMPLE_LOADER_FUNCTION(vaReleaseBufferHandle)
    , SIMPLE_LOADER_FUNCTION(vaMaxNumEntrypoints)
    , SIMPLE_LOADER_FUNCTION(vaQueryConfigEntrypoints)
    , SIMPLE_LOADER_FUNCTION(vaGetConfigAttributes)
    , SIMPLE_LOADER_FUNCTION(vaCreateConfig)
    , SIMPLE_LOADER_FUNCTION(vaCreateContext)
    , SIMPLE_LOADER_FUNCTION(vaDestroyConfig)
    , SIMPLE_LOADER_FUNCTION(vaDestroyContext)
{
}

VA_Proxy::~VA_Proxy()
{}

#endif

#if defined(LIBVA_DRM_SUPPORT)
DRM_Proxy::DRM_Proxy()
    : lib("libdrm.so.2")
    , SIMPLE_LOADER_FUNCTION(drmIoctl)
    , SIMPLE_LOADER_FUNCTION(drmModeAddFB)
    , SIMPLE_LOADER_FUNCTION(drmModeAddFB2WithModifiers)
    , SIMPLE_LOADER_FUNCTION(drmModeFreeConnector)
    , SIMPLE_LOADER_FUNCTION(drmModeFreeCrtc)
    , SIMPLE_LOADER_FUNCTION(drmModeFreeEncoder)
    , SIMPLE_LOADER_FUNCTION(drmModeFreePlane)
    , SIMPLE_LOADER_FUNCTION(drmModeFreePlaneResources)
    , SIMPLE_LOADER_FUNCTION(drmModeFreeResources)
    , SIMPLE_LOADER_FUNCTION(drmModeGetConnector)
    , SIMPLE_LOADER_FUNCTION(drmModeGetCrtc)
    , SIMPLE_LOADER_FUNCTION(drmModeGetEncoder)
    , SIMPLE_LOADER_FUNCTION(drmModeGetPlane)
    , SIMPLE_LOADER_FUNCTION(drmModeGetPlaneResources)
    , SIMPLE_LOADER_FUNCTION(drmModeGetResources)
    , SIMPLE_LOADER_FUNCTION(drmModeRmFB)
    , SIMPLE_LOADER_FUNCTION(drmModeSetCrtc)
    , SIMPLE_LOADER_FUNCTION(drmSetMaster)
    , SIMPLE_LOADER_FUNCTION(drmDropMaster)
    , SIMPLE_LOADER_FUNCTION(drmModeSetPlane)
{
}

DrmIntel_Proxy::~DrmIntel_Proxy()
{}

DrmIntel_Proxy::DrmIntel_Proxy()
    : lib("libdrm_intel.so.1")
    , SIMPLE_LOADER_FUNCTION(drm_intel_bo_gem_create_from_prime)
    , SIMPLE_LOADER_FUNCTION(drm_intel_bo_unreference)
    , SIMPLE_LOADER_FUNCTION(drm_intel_bufmgr_gem_init)
    , SIMPLE_LOADER_FUNCTION(drm_intel_bufmgr_destroy)
#if defined(X11_DRI3_SUPPORT)
    , SIMPLE_LOADER_FUNCTION(drm_intel_bo_gem_export_to_prime)
#endif
{
}

DRM_Proxy::~DRM_Proxy()
{}

VA_DRMProxy::VA_DRMProxy()
    : lib("libva-drm.so.2")
    , SIMPLE_LOADER_FUNCTION(vaGetDisplayDRM)
{
}

VA_DRMProxy::~VA_DRMProxy()
{}

#if defined(X11_DRI3_SUPPORT)
XCB_Dri3_Proxy::XCB_Dri3_Proxy()
    : lib("libxcb-dri3.so.0")
    , SIMPLE_LOADER_FUNCTION(xcb_dri3_pixmap_from_buffer_checked)
{
}

XCB_Dri3_Proxy::~XCB_Dri3_Proxy()
{}

Xcb_Proxy::Xcb_Proxy()
    : lib("libxcb.so.1")
    , SIMPLE_LOADER_FUNCTION(xcb_generate_id)
    , SIMPLE_LOADER_FUNCTION(xcb_free_pixmap)
    , SIMPLE_LOADER_FUNCTION(xcb_flush)
    , SIMPLE_LOADER_FUNCTION(xcb_request_check)
{
}

Xcb_Proxy::~Xcb_Proxy()
{}

X11_Xcb_Proxy::X11_Xcb_Proxy()
    : lib("libX11-xcb.so.1")
    , SIMPLE_LOADER_FUNCTION(XGetXCBConnection)
{
}

X11_Xcb_Proxy::~X11_Xcb_Proxy()
{}

Xcbpresent_Proxy::Xcbpresent_Proxy()
    : lib("libxcb-present.so.0")
    , SIMPLE_LOADER_FUNCTION(xcb_present_pixmap_checked)
{
}

Xcbpresent_Proxy::~Xcbpresent_Proxy()
{}
#endif // X11_DRI3_SUPPORT
#endif

#if defined(LIBVA_WAYLAND_SUPPORT)

VA_WaylandClientProxy::VA_WaylandClientProxy()
    : lib("libmfx_wayland.so")
    , SIMPLE_LOADER_FUNCTION(WaylandCreate)
{
}

VA_WaylandClientProxy::~VA_WaylandClientProxy()
{}

#endif // LIBVA_WAYLAND_SUPPORT

#if defined(LIBVA_X11_SUPPORT)
VA_X11Proxy::VA_X11Proxy()
    : lib("libva-x11.so.2")
    , SIMPLE_LOADER_FUNCTION(vaGetDisplay)
    , SIMPLE_LOADER_FUNCTION(vaPutSurface)
{
}

VA_X11Proxy::~VA_X11Proxy()
{}

XLib_Proxy::XLib_Proxy()
    : lib("libX11.so.6")
    , SIMPLE_LOADER_FUNCTION(XOpenDisplay)
    , SIMPLE_LOADER_FUNCTION(XCloseDisplay)
    , SIMPLE_LOADER_FUNCTION(XCreateSimpleWindow)
    , SIMPLE_LOADER_FUNCTION(XMapWindow)
    , SIMPLE_LOADER_FUNCTION(XSync)
    , SIMPLE_LOADER_FUNCTION(XDestroyWindow)
    , SIMPLE_LOADER_FUNCTION(XResizeWindow)
#if defined(X11_DRI3_SUPPORT)
    , SIMPLE_LOADER_FUNCTION(XGetGeometry)
#endif // X11_DRI3_SUPPORT
{}

XLib_Proxy::~XLib_Proxy()
{}


#endif

#undef SIMPLE_LOADER_FUNCTION

} // MfxLoader

mfxStatus va_to_mfx_status(VAStatus va_res)
{
    mfxStatus mfxRes = MFX_ERR_NONE;

    switch (va_res)
    {
    case VA_STATUS_SUCCESS:
        mfxRes = MFX_ERR_NONE;
        break;
    case VA_STATUS_ERROR_ALLOCATION_FAILED:
        mfxRes = MFX_ERR_MEMORY_ALLOC;
        break;
    case VA_STATUS_ERROR_ATTR_NOT_SUPPORTED:
    case VA_STATUS_ERROR_UNSUPPORTED_PROFILE:
    case VA_STATUS_ERROR_UNSUPPORTED_ENTRYPOINT:
    case VA_STATUS_ERROR_UNSUPPORTED_RT_FORMAT:
    case VA_STATUS_ERROR_UNSUPPORTED_BUFFERTYPE:
    case VA_STATUS_ERROR_FLAG_NOT_SUPPORTED:
    case VA_STATUS_ERROR_RESOLUTION_NOT_SUPPORTED:
        mfxRes = MFX_ERR_UNSUPPORTED;
        break;
    case VA_STATUS_ERROR_INVALID_DISPLAY:
    case VA_STATUS_ERROR_INVALID_CONFIG:
    case VA_STATUS_ERROR_INVALID_CONTEXT:
    case VA_STATUS_ERROR_INVALID_SURFACE:
    case VA_STATUS_ERROR_INVALID_BUFFER:
    case VA_STATUS_ERROR_INVALID_IMAGE:
    case VA_STATUS_ERROR_INVALID_SUBPICTURE:
        mfxRes = MFX_ERR_NOT_INITIALIZED;
        break;
    case VA_STATUS_ERROR_INVALID_PARAMETER:
        mfxRes = MFX_ERR_INVALID_VIDEO_PARAM;
    default:
        mfxRes = MFX_ERR_UNKNOWN;
        break;
    }
    return mfxRes;
}

#if defined(LIBVA_DRM_SUPPORT) || defined(LIBVA_X11_SUPPORT)
CLibVA* CreateLibVA(const std::string& devicePath, int type)
{
    CLibVA * libva = NULL;
    switch (type)
    {
    case MFX_LIBVA_DRM:
#if defined(LIBVA_DRM_SUPPORT)
        try
        {
            libva = new DRMLibVA(devicePath, type);
        }
        catch (std::exception&)
        {
            libva = 0;
        }
#endif
        break;

    case MFX_LIBVA_X11:
#if defined(LIBVA_X11_SUPPORT)
        try
        {
            libva = new X11LibVA;
        }
        catch (std::exception&)
        {
            libva = NULL;
        }
#endif
        break;

    case MFX_LIBVA_AUTO:
#if defined(LIBVA_X11_SUPPORT)
        try
        {
            libva = new X11LibVA;
        }
        catch (std::exception&)
        {
            libva = NULL;
        }
#endif
#if defined(LIBVA_DRM_SUPPORT)
        if (!libva)
        {
            try
            {
                libva = new DRMLibVA(devicePath, type);
            }
            catch (std::exception&)
            {
                libva = NULL;
            }
        }
#endif
        break;
    } // switch(type)

    return libva;
}
#endif // #if defined(LIBVA_DRM_SUPPORT) || defined(LIBVA_X11_SUPPORT)

#if defined(LIBVA_X11_SUPPORT)

struct AcquireCtx
{
    VAImage image;
};

VAStatus CLibVA::AcquireVASurface(
    void** pctx,
    VADisplay dpy1,
    VASurfaceID srf1,
    VADisplay dpy2,
    VASurfaceID* srf2)
{
    if (!pctx || !srf2) return VA_STATUS_ERROR_OPERATION_FAILED;

    if (dpy1 == dpy2) {
        *srf2 = srf1;
        return VA_STATUS_SUCCESS;
    }

    AcquireCtx* ctx;
    uintptr_t handle=0;
    VAStatus va_res;
    VASurfaceAttrib attribs[2];
    VASurfaceAttribExternalBuffers extsrf;
    VABufferInfo bufferInfo;
    uint32_t memtype = VA_SURFACE_ATTRIB_MEM_TYPE_DRM_PRIME;

    MSDK_ZERO_MEMORY(attribs);
    MSDK_ZERO_MEMORY(extsrf);
    MSDK_ZERO_MEMORY(bufferInfo);
    extsrf.num_buffers = 1;
    extsrf.buffers = &handle;

    attribs[0].type = (VASurfaceAttribType)VASurfaceAttribMemoryType;
    attribs[0].flags = VA_SURFACE_ATTRIB_SETTABLE;
    attribs[0].value.type = VAGenericValueTypeInteger;
    attribs[0].value.value.i = memtype;

    attribs[1].type = (VASurfaceAttribType)VASurfaceAttribExternalBufferDescriptor;
    attribs[1].flags = VA_SURFACE_ATTRIB_SETTABLE;
    attribs[1].value.type = VAGenericValueTypePointer;
    attribs[1].value.value.p = &extsrf;

    ctx = (AcquireCtx*)calloc(1, sizeof(AcquireCtx));
    if (!ctx) return VA_STATUS_ERROR_OPERATION_FAILED;

    va_res = m_libva.vaDeriveImage(dpy1, srf1, &ctx->image);
    if (VA_STATUS_SUCCESS != va_res) {
        free(ctx);
        return va_res;
    }

    va_res = m_libva.vaAcquireBufferHandle(dpy1, ctx->image.buf, &bufferInfo);
    if (VA_STATUS_SUCCESS != va_res) {
        m_libva.vaDestroyImage(dpy1, ctx->image.image_id);
        free(ctx);
        return va_res;
    }

    extsrf.width = ctx->image.width;
    extsrf.height = ctx->image.height;
    extsrf.num_planes = ctx->image.num_planes;
    extsrf.pixel_format = ctx->image.format.fourcc;
    for (int i=0; i < 3; ++i) {
        extsrf.pitches[i] = ctx->image.pitches[i];
        extsrf.offsets[i] = ctx->image.offsets[i];
    }
    extsrf.data_size = ctx->image.data_size;
    extsrf.flags = memtype;
    extsrf.buffers[0] = bufferInfo.handle;

    va_res = m_libva.vaCreateSurfaces(dpy2,
        VA_RT_FORMAT_YUV420,
        extsrf.width, extsrf.height,
        srf2, 1, attribs, 2);
    if (VA_STATUS_SUCCESS != va_res) {
        m_libva.vaDestroyImage(dpy1, ctx->image.image_id);
        free(ctx);
        return va_res;
    }

    *pctx = ctx;

    return VA_STATUS_SUCCESS;
}

void CLibVA::ReleaseVASurface(
    void* actx,
    VADisplay dpy1,
    VASurfaceID /*srf1*/,
    VADisplay dpy2,
    VASurfaceID srf2)
{
    if (dpy1 != dpy2) {
        AcquireCtx* ctx = (AcquireCtx*)actx;
        if (ctx) {
            m_libva.vaDestroySurfaces(dpy2, &srf2, 1);
            m_libva.vaReleaseBufferHandle(dpy1, ctx->image.buf);
            m_libva.vaDestroyImage(dpy1, ctx->image.image_id);
            free(ctx);
        }
    }
}

#endif //LIBVA_X11_SUPPORT

#endif // #ifdef LIBVA_SUPPORT
