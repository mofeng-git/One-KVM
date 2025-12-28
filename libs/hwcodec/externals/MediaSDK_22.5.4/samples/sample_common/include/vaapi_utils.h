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

#ifndef __VAAPI_UTILS_H__
#define __VAAPI_UTILS_H__

#ifdef LIBVA_SUPPORT

#include <va/va.h>
#include <va/va_drmcommon.h>

#if defined(LIBVA_DRM_SUPPORT)
#include <xf86drm.h>
#include <xf86drmMode.h>
#include <libdrm/intel_bufmgr.h>
#include <va/va_drm.h>
#endif
#if defined(LIBVA_X11_SUPPORT)
#include <va/va_x11.h>
#if defined(X11_DRI3_SUPPORT)
#include <xcb/dri3.h>
#include <xcb/present.h>
#endif // X11_DRI3_SUPPORT
#endif
#include "sample_defs.h"
#include "sample_utils.h"
#include "vm/thread_defs.h"

namespace MfxLoader
{

    class SimpleLoader
    {
    public:
        SimpleLoader(const char * name);

        void * GetFunction(const char * name);

        ~SimpleLoader();

    private:
        SimpleLoader(SimpleLoader&);
        void operator=(SimpleLoader&);

        void * so_handle;
    };

#ifdef LIBVA_SUPPORT
    class VA_Proxy
    {
    private:
        SimpleLoader lib; // should appear first in member list

    public:
        typedef VAStatus (*vaInitialize_type)(VADisplay, int *, int *);
        typedef VAStatus (*vaTerminate_type)(VADisplay);
        typedef VAStatus (*vaCreateSurfaces_type)(VADisplay, unsigned int,
            unsigned int, unsigned int, VASurfaceID *, unsigned int,
            VASurfaceAttrib *, unsigned int);
        typedef VAStatus (*vaDestroySurfaces_type)(VADisplay, VASurfaceID *, int);
        typedef VAStatus (*vaCreateBuffer_type)(VADisplay, VAContextID,
            VABufferType, unsigned int, unsigned int, void *, VABufferID *);
        typedef VAStatus (*vaDestroyBuffer_type)(VADisplay, VABufferID);
        typedef VAStatus (*vaMapBuffer_type)(VADisplay, VABufferID, void **pbuf);
        typedef VAStatus (*vaUnmapBuffer_type)(VADisplay, VABufferID);
        typedef VAStatus (*vaSyncSurface_type)(VADisplay, VASurfaceID);
        typedef VAStatus (*vaDeriveImage_type)(VADisplay, VASurfaceID, VAImage *);
        typedef VAStatus (*vaDestroyImage_type)(VADisplay, VAImageID);
        typedef VAStatus (*vaGetLibFunc_type)(VADisplay, const char *func);
        typedef VAStatus (*vaAcquireBufferHandle_type)(VADisplay, VABufferID, VABufferInfo *);
        typedef VAStatus (*vaReleaseBufferHandle_type)(VADisplay, VABufferID);
        typedef VAStatus (*vaMaxNumEntrypoints_type)(VADisplay dpy);
        typedef VAStatus (*vaQueryConfigEntrypoints_type)(VADisplay dpy,
                                                          VAProfile profile,
                                                          VAEntrypoint *entrypoint_list,
                                                          int *num_entrypoints);
        typedef VAStatus (*vaGetConfigAttributes_type)(VADisplay dpy,
                                                       VAProfile profile,
                                                       VAEntrypoint entrypoint,
                                                       VAConfigAttrib *attrib_list,
                                                       int num_attribs);
        typedef VAStatus (*vaCreateConfig_type)(VADisplay dpy,
                                                VAProfile profile,
                                                VAEntrypoint entrypoint,
                                                VAConfigAttrib *attrib_list,
                                                int num_attribs,
                                                VAConfigID *config_id);

        typedef VAStatus (*vaCreateContext_type)(VADisplay dpy,
                                                 VAConfigID config_id,
                                                 int picture_width,
                                                 int picture_height,
                                                 int flag,
                                                 VASurfaceID *render_targets,
                                                 int num_render_targets,
                                                 VAContextID *context);
        typedef VAStatus (*vaDestroyConfig_type) (VADisplay dpy,
                                                  VAConfigID config_id);
        typedef VAStatus (*vaDestroyContext_type) (VADisplay dpy,
                                                   VAContextID context);

        VA_Proxy();
        ~VA_Proxy();

        const vaInitialize_type             vaInitialize;
        const vaTerminate_type              vaTerminate;
        const vaCreateSurfaces_type         vaCreateSurfaces;
        const vaDestroySurfaces_type        vaDestroySurfaces;
        const vaCreateBuffer_type           vaCreateBuffer;
        const vaDestroyBuffer_type          vaDestroyBuffer;
        const vaMapBuffer_type              vaMapBuffer;
        const vaUnmapBuffer_type            vaUnmapBuffer;
        const vaSyncSurface_type            vaSyncSurface;
        const vaDeriveImage_type            vaDeriveImage;
        const vaDestroyImage_type           vaDestroyImage;
        const vaGetLibFunc_type             vaGetLibFunc;
        const vaAcquireBufferHandle_type    vaAcquireBufferHandle;
        const vaReleaseBufferHandle_type    vaReleaseBufferHandle;
        const vaMaxNumEntrypoints_type      vaMaxNumEntrypoints;
        const vaQueryConfigEntrypoints_type vaQueryConfigEntrypoints;
        const vaGetConfigAttributes_type    vaGetConfigAttributes;
        const vaCreateConfig_type           vaCreateConfig;
        const vaCreateContext_type          vaCreateContext;
        const vaDestroyConfig_type          vaDestroyConfig;
        const vaDestroyContext_type         vaDestroyContext;
    };
#endif

#if defined(LIBVA_DRM_SUPPORT)
    class DRM_Proxy
    {
    private:
        SimpleLoader lib; // should appear first in member list

    public:
        typedef int (*drmIoctl_type)(int fd, unsigned long request, void *arg);
        typedef int (*drmModeAddFB_type)(
          int fd, uint32_t width, uint32_t height, uint8_t depth,
          uint8_t bpp, uint32_t pitch, uint32_t bo_handle,
          uint32_t *buf_id);
        typedef int (*drmModeAddFB2WithModifiers_type)(int fd, uint32_t width, uint32_t height, uint32_t pixel_format,
          uint32_t bo_handles[4], uint32_t pitches[4], uint32_t offsets[4], uint64_t modifier[4],
          uint32_t *buf_id, uint32_t flags);
        typedef void (*drmModeFreeConnector_type)( drmModeConnectorPtr ptr );
        typedef void (*drmModeFreeCrtc_type)( drmModeCrtcPtr ptr );
        typedef void (*drmModeFreeEncoder_type)( drmModeEncoderPtr ptr );
        typedef void (*drmModeFreePlane_type)( drmModePlanePtr ptr );
        typedef void (*drmModeFreePlaneResources_type)(drmModePlaneResPtr ptr);
        typedef void (*drmModeFreeResources_type)( drmModeResPtr ptr );
        typedef drmModeConnectorPtr (*drmModeGetConnector_type)(
          int fd, uint32_t connectorId);
        typedef drmModeCrtcPtr (*drmModeGetCrtc_type)(int fd, uint32_t crtcId);
        typedef drmModeEncoderPtr (*drmModeGetEncoder_type)(int fd, uint32_t encoder_id);
        typedef drmModePlanePtr (*drmModeGetPlane_type)(int fd, uint32_t plane_id);
        typedef drmModePlaneResPtr (*drmModeGetPlaneResources_type)(int fd);
        typedef drmModeResPtr (*drmModeGetResources_type)(int fd);
        typedef int (*drmModeRmFB_type)(int fd, uint32_t bufferId);
        typedef int (*drmModeSetCrtc_type)(
          int fd, uint32_t crtcId, uint32_t bufferId,
          uint32_t x, uint32_t y, uint32_t *connectors, int count,
          drmModeModeInfoPtr mode);
        typedef int (*drmSetMaster_type)(int fd);
        typedef int (*drmDropMaster_type)(int fd);
        typedef int (*drmModeSetPlane_type)(
          int fd, uint32_t plane_id, uint32_t crtc_id,
          uint32_t fb_id, uint32_t flags,
          int32_t crtc_x, int32_t crtc_y,
          uint32_t crtc_w, uint32_t crtc_h,
          uint32_t src_x, uint32_t src_y,
          uint32_t src_w, uint32_t src_h);

        DRM_Proxy();
        ~DRM_Proxy();

#define __DECLARE(name) const name ## _type name
        __DECLARE(drmIoctl);
        __DECLARE(drmModeAddFB);
        __DECLARE(drmModeAddFB2WithModifiers);
        __DECLARE(drmModeFreeConnector);
        __DECLARE(drmModeFreeCrtc);
        __DECLARE(drmModeFreeEncoder);
        __DECLARE(drmModeFreePlane);
        __DECLARE(drmModeFreePlaneResources);
        __DECLARE(drmModeFreeResources);
        __DECLARE(drmModeGetConnector);
        __DECLARE(drmModeGetCrtc);
        __DECLARE(drmModeGetEncoder);
        __DECLARE(drmModeGetPlane);
        __DECLARE(drmModeGetPlaneResources);
        __DECLARE(drmModeGetResources);
        __DECLARE(drmModeRmFB);
        __DECLARE(drmModeSetCrtc);
        __DECLARE(drmSetMaster);
        __DECLARE(drmDropMaster);
        __DECLARE(drmModeSetPlane);
#undef __DECLARE
    };

    class DrmIntel_Proxy
    {
    private:
        SimpleLoader lib; // should appear first in member list

    public:
        typedef drm_intel_bo* (*drm_intel_bo_gem_create_from_prime_type)(
          drm_intel_bufmgr *bufmgr, int prime_fd, int size);
        typedef void (*drm_intel_bo_unreference_type)(drm_intel_bo *bo);
        typedef drm_intel_bufmgr* (*drm_intel_bufmgr_gem_init_type)(int fd, int batch_size);
        typedef int (*drm_intel_bo_gem_export_to_prime_type) (drm_intel_bo *, int *);
        typedef void (*drm_intel_bufmgr_destroy_type)(drm_intel_bufmgr*);

        DrmIntel_Proxy();
        ~DrmIntel_Proxy();

#define __DECLARE(name) const name ## _type name
        __DECLARE(drm_intel_bo_gem_create_from_prime);
        __DECLARE(drm_intel_bo_unreference);
        __DECLARE(drm_intel_bufmgr_gem_init);
        __DECLARE(drm_intel_bufmgr_destroy);
#if defined(X11_DRI3_SUPPORT)
        __DECLARE(drm_intel_bo_gem_export_to_prime);
#endif

#undef __DECLARE
    };

    class VA_DRMProxy
    {
    private:
        SimpleLoader lib; // should appear first in member list

    public:
        typedef VADisplay (*vaGetDisplayDRM_type)(int);


        VA_DRMProxy();
        ~VA_DRMProxy();

        const vaGetDisplayDRM_type vaGetDisplayDRM;
    };
#endif

#if defined (LIBVA_WAYLAND_SUPPORT)

    class Wayland;

    class VA_WaylandClientProxy
    {
    private:
        SimpleLoader lib; // should appear first in member list

    public:
        typedef    Wayland* (*WaylandCreate_type)(void);

        VA_WaylandClientProxy();
        ~VA_WaylandClientProxy();

        const WaylandCreate_type WaylandCreate;
    };

#endif // LIBVA_WAYLAND_SUPPORT

#if defined(LIBVA_X11_SUPPORT)
    class VA_X11Proxy
    {
    private:
        SimpleLoader lib; // should appear first in member list

    public:
        typedef VADisplay (*vaGetDisplay_type)(Display*);
        typedef VAStatus (*vaPutSurface_type)(
            VADisplay, VASurfaceID,
            Drawable,
            short, short,
            unsigned short, unsigned short,
            short, short,
            unsigned short, unsigned short,
            VARectangle *,
            unsigned int, unsigned int);

        VA_X11Proxy();
        ~VA_X11Proxy();

        const vaGetDisplay_type vaGetDisplay;
        const vaPutSurface_type vaPutSurface;
    };

    class XLib_Proxy
    {
    private:
        SimpleLoader lib; // should appear first in member list

    public:
        typedef Display* (*XOpenDisplay_type) (const char*);
        typedef int (*XCloseDisplay_type)(Display*);
        typedef Window (*XCreateSimpleWindow_type)(Display *,
            Window, int, int,
            unsigned int, unsigned int,
            unsigned int, unsigned long,
            unsigned long);
        typedef int (*XMapWindow_type)(Display*, Window);
        typedef int (*XSync_type)(Display*, Bool);
        typedef int (*XDestroyWindow_type)(Display*, Window);
        typedef int (*XResizeWindow_type)(Display *, Window, unsigned int, unsigned int);
#if defined(X11_DRI3_SUPPORT)
        typedef Status (*XGetGeometry_type)(Display *, Drawable, Window *,
                            int *, int *, unsigned int *, unsigned int *,
                            unsigned int *, unsigned int *);
#endif // X11_DRI3_SUPPORT
        XLib_Proxy();
        ~XLib_Proxy();

        const XOpenDisplay_type        XOpenDisplay;
        const XCloseDisplay_type       XCloseDisplay;
        const XCreateSimpleWindow_type XCreateSimpleWindow;
        const XMapWindow_type          XMapWindow;
        const XSync_type               XSync;
        const XDestroyWindow_type      XDestroyWindow;
        const XResizeWindow_type       XResizeWindow;
#if defined(X11_DRI3_SUPPORT)
        const XGetGeometry_type        XGetGeometry;
#endif // X11_DRI3_SUPPORT
    };

#if defined(X11_DRI3_SUPPORT)

    class XCB_Dri3_Proxy
    {
        private:
        SimpleLoader lib; // should appear first in member list

        public:
        typedef xcb_void_cookie_t (*xcb_dri3_pixmap_from_buffer_type) (xcb_connection_t *,
                                    xcb_pixmap_t,
                                    xcb_drawable_t,
                                    uint32_t,
                                    uint16_t,
                                    uint16_t,
                                    uint16_t,
                                    uint8_t,
                                    uint8_t,
                                    int32_t);
        typedef xcb_dri3_pixmap_from_buffer_type xcb_dri3_pixmap_from_buffer_checked_type;

        XCB_Dri3_Proxy();
        ~XCB_Dri3_Proxy();

        const xcb_dri3_pixmap_from_buffer_checked_type  xcb_dri3_pixmap_from_buffer_checked;
    };

    class Xcb_Proxy
    {
        private:
        SimpleLoader lib; // should appear first in member list

        public:
        typedef uint32_t (*xcb_generate_id_type) (xcb_connection_t *);
        typedef xcb_void_cookie_t (*xcb_free_pixmap_type) (xcb_connection_t *, xcb_pixmap_t);
        typedef int (*xcb_flush_type)(xcb_connection_t *);
        typedef xcb_generic_error_t* (*xcb_request_check_type)(xcb_connection_t *c, xcb_void_cookie_t cookie);

        Xcb_Proxy();
        ~Xcb_Proxy();

        const xcb_generate_id_type   xcb_generate_id;
        const xcb_free_pixmap_type   xcb_free_pixmap;
        const xcb_flush_type         xcb_flush;
        const xcb_request_check_type xcb_request_check;
    };

    class X11_Xcb_Proxy
    {
        private:
        SimpleLoader lib; // should appear first in member list

        public:
        typedef xcb_connection_t * (*XGetXCBConnection_type) (Display *dpy);

        X11_Xcb_Proxy();
        ~X11_Xcb_Proxy();

        const XGetXCBConnection_type XGetXCBConnection;
    };

    class Xcbpresent_Proxy
    {
        private:
        SimpleLoader lib;

        public:
        typedef xcb_void_cookie_t (*xcb_present_pixmap_type) (xcb_connection_t *,
                                    xcb_window_t,
                                    xcb_pixmap_t,
                                    uint32_t,
                                    xcb_xfixes_region_t,
                                    xcb_xfixes_region_t,
                                    int16_t,
                                    int16_t,
                                    xcb_randr_crtc_t,
                                    xcb_sync_fence_t,
                                    xcb_sync_fence_t,
                                    uint32_t,
                                    uint64_t,
                                    uint64_t,
                                    uint64_t,
                                    uint32_t,
                                    const xcb_present_notify_t *);
        typedef xcb_present_pixmap_type xcb_present_pixmap_checked_type;

        Xcbpresent_Proxy();
        ~Xcbpresent_Proxy();

        const xcb_present_pixmap_checked_type xcb_present_pixmap_checked;
    };

#endif // X11_DRI3_SUPPORT
#endif

} // namespace MfxLoader


class CLibVA
{
public:
    virtual ~CLibVA(void) {};

    VAStatus AcquireVASurface(
        void** pctx,
        VADisplay dpy1,
        VASurfaceID srf1,
        VADisplay dpy2,
        VASurfaceID* srf2);
    void ReleaseVASurface(
        void* actx,
        VADisplay dpy1,
        VASurfaceID /*srf1*/,
        VADisplay dpy2,
        VASurfaceID srf2);

    inline int getBackendType() { return m_type; }
    VADisplay GetVADisplay() { return m_va_dpy; }
    const MfxLoader::VA_Proxy m_libva;

protected:
    CLibVA(int type)
      : m_type(type)
      , m_va_dpy(NULL)
    {}
    int m_type;
    VADisplay m_va_dpy;

private:
    DISALLOW_COPY_AND_ASSIGN(CLibVA);
};

CLibVA* CreateLibVA(const std::string& devicePath = "", int type = MFX_LIBVA_DRM);

VAStatus AcquireVASurface(void** ctx, VADisplay dpy1, VASurfaceID srf1, VADisplay dpy2, VASurfaceID* srf2);
void ReleaseVASurface(void* actx, VADisplay dpy1, VASurfaceID srf1, VADisplay dpy2, VASurfaceID srf2);

mfxStatus va_to_mfx_status(VAStatus va_res);

#endif // #ifdef LIBVA_SUPPORT

#endif // #ifndef __VAAPI_UTILS_H__
