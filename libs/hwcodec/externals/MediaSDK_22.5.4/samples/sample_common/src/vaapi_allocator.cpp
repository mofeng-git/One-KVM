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

#if defined(LIBVA_SUPPORT)

#include <stdio.h>
#include <assert.h>

#include "vaapi_allocator.h"
#include "vaapi_utils.h"

enum {
    MFX_FOURCC_VP8_NV12    = MFX_MAKEFOURCC('V','P','8','N'),
    MFX_FOURCC_VP8_MBDATA  = MFX_MAKEFOURCC('V','P','8','M'),
    MFX_FOURCC_VP8_SEGMAP  = MFX_MAKEFOURCC('V','P','8','S'),
};

unsigned int ConvertMfxFourccToVAFormat(mfxU32 fourcc)
{
    switch (fourcc)
    {
    case MFX_FOURCC_NV12:
        return VA_FOURCC_NV12;
    case MFX_FOURCC_YUY2:
        return VA_FOURCC_YUY2;
    case MFX_FOURCC_UYVY:
        return VA_FOURCC_UYVY;
    case MFX_FOURCC_YV12:
        return VA_FOURCC_YV12;
#if (MFX_VERSION >= 1028)
    case MFX_FOURCC_RGB565:
        return VA_FOURCC_RGB565;
#endif
    case MFX_FOURCC_RGB4:
        return VA_FOURCC_ARGB;
    case MFX_FOURCC_BGR4:
        return VA_FOURCC_ABGR;
    case MFX_FOURCC_RGBP:
        return VA_FOURCC_RGBP;
    case MFX_FOURCC_P8:
        return VA_FOURCC_P208;
    case MFX_FOURCC_P010:
        return VA_FOURCC_P010;
    case MFX_FOURCC_A2RGB10:
        return VA_FOURCC_ARGB;  // rt format will be VA_RT_FORMAT_RGB32_10BPP
    case MFX_FOURCC_AYUV:
        return VA_FOURCC_AYUV;
#if (MFX_VERSION >= 1027)
    case MFX_FOURCC_Y210:
        return VA_FOURCC_Y210;
    case MFX_FOURCC_Y410:
        return VA_FOURCC_Y410;
#endif
#if (MFX_VERSION >= 1031)
    case MFX_FOURCC_P016:
        return VA_FOURCC_P016;
    case MFX_FOURCC_Y216:
        return VA_FOURCC_Y216;
    case MFX_FOURCC_Y416:
        return VA_FOURCC_Y416;
#endif
    default:
        assert(!"unsupported fourcc");
        return 0;
    }
}

unsigned int ConvertVP8FourccToMfxFourcc(mfxU32 fourcc)
{
    switch (fourcc)
    {
    case MFX_FOURCC_VP8_NV12:
    case MFX_FOURCC_VP8_MBDATA:
        return MFX_FOURCC_NV12;
    case MFX_FOURCC_VP8_SEGMAP:
        return MFX_FOURCC_P8;

    default:
        return fourcc;
    }
}

vaapiFrameAllocator::vaapiFrameAllocator()
    : m_dpy(0)
    , m_libva(new MfxLoader::VA_Proxy)
    , m_export_mode(vaapiAllocatorParams::DONOT_EXPORT)
    , m_exporter(NULL)
{
}

vaapiFrameAllocator::~vaapiFrameAllocator()
{
    Close();
    delete m_libva;
}

mfxStatus vaapiFrameAllocator::Init(mfxAllocatorParams *pParams)
{
    vaapiAllocatorParams* p_vaapiParams = dynamic_cast<vaapiAllocatorParams*>(pParams);

    if ((NULL == p_vaapiParams) || (NULL == p_vaapiParams->m_dpy))
        return MFX_ERR_NOT_INITIALIZED;

    if ((p_vaapiParams->m_export_mode != vaapiAllocatorParams::DONOT_EXPORT) &&
        !(p_vaapiParams->m_export_mode & vaapiAllocatorParams::FLINK) &&
        !(p_vaapiParams->m_export_mode & vaapiAllocatorParams::PRIME) &&
        !(p_vaapiParams->m_export_mode & vaapiAllocatorParams::CUSTOM))
      return MFX_ERR_UNSUPPORTED;
    if ((p_vaapiParams->m_export_mode & vaapiAllocatorParams::CUSTOM) &&
        !p_vaapiParams->m_exporter)
      return MFX_ERR_UNSUPPORTED;

    m_dpy = p_vaapiParams->m_dpy;
    m_export_mode = p_vaapiParams->m_export_mode;
    m_exporter = p_vaapiParams->m_exporter;
    return MFX_ERR_NONE;
}

mfxStatus vaapiFrameAllocator::CheckRequestType(mfxFrameAllocRequest *request)
{
    mfxStatus sts = BaseFrameAllocator::CheckRequestType(request);
    if (MFX_ERR_NONE != sts)
        return sts;

    if ((request->Type & (MFX_MEMTYPE_VIDEO_MEMORY_DECODER_TARGET | MFX_MEMTYPE_VIDEO_MEMORY_PROCESSOR_TARGET)) != 0)
        return MFX_ERR_NONE;
    else
        return MFX_ERR_UNSUPPORTED;
}

mfxStatus vaapiFrameAllocator::Close()
{
    return BaseFrameAllocator::Close();
}

static mfxStatus GetVAFourcc(mfxU32 fourcc, unsigned int &va_fourcc)
{
    // VP8 hybrid driver has weird requirements for allocation of surfaces/buffers for VP8 encoding
    // to comply with them additional logic is required to support regular and VP8 hybrid allocation pathes
    mfxU32 mfx_fourcc = ConvertVP8FourccToMfxFourcc(fourcc);
    va_fourcc = ConvertMfxFourccToVAFormat(mfx_fourcc);
    if (!va_fourcc || ((VA_FOURCC_NV12 != va_fourcc) &&
        (VA_FOURCC_YV12 != va_fourcc) &&
        (VA_FOURCC_YUY2 != va_fourcc) &&
        (VA_FOURCC_ARGB != va_fourcc) &&
        (VA_FOURCC_ABGR != va_fourcc) &&
        (VA_FOURCC_RGBP != va_fourcc) &&
        (VA_FOURCC_P208 != va_fourcc) &&
        (VA_FOURCC_P010 != va_fourcc) &&
        (VA_FOURCC_YUY2 != va_fourcc) &&
#if (MFX_VERSION >= 1027)
        (VA_FOURCC_Y210 != va_fourcc) &&
        (VA_FOURCC_Y410 != va_fourcc) &&
#endif
#if (MFX_VERSION >= 1028)
        (VA_FOURCC_RGB565 != va_fourcc) &&
#endif
#if (MFX_VERSION >= 1031)
        (VA_FOURCC_P016 != va_fourcc) &&
        (VA_FOURCC_Y216 != va_fourcc) &&
        (VA_FOURCC_Y416 != va_fourcc) &&
#endif
        (VA_FOURCC_AYUV != va_fourcc)))
    {
        return MFX_ERR_MEMORY_ALLOC;
    }

    return MFX_ERR_NONE;
}

mfxStatus vaapiFrameAllocator::ReallocImpl(mfxMemId mid, const mfxFrameInfo *info, mfxU16 memType, mfxMemId *midOut)
{
    if (!info || !midOut) return MFX_ERR_NULL_PTR;

    mfxStatus mfx_res = MFX_ERR_NONE;
    VAStatus  va_res = VA_STATUS_SUCCESS;
    unsigned int va_fourcc = 0;
    mfxU32 fourcc = info->FourCC;

    mfx_res = GetVAFourcc(fourcc, va_fourcc);
    if (MFX_ERR_NONE != mfx_res)
        return mfx_res;

    mfxU32 Width = info->Width;
    mfxU32 Height = info->Height;

    if (VA_FOURCC_P208 == va_fourcc)
        return MFX_ERR_UNSUPPORTED;

    VASurfaceID surfaces[1];
    VASurfaceAttrib attrib[2];
    vaapiMemId *vaapiMid = (vaapiMemId *)mid;
    surfaces[0] = *vaapiMid->m_surface;
    m_libva->vaDestroySurfaces(m_dpy, surfaces, 1);

    unsigned int format;
    int attrCnt = 0;

    attrib[attrCnt].type = VASurfaceAttribPixelFormat;
    attrib[attrCnt].flags = VA_SURFACE_ATTRIB_SETTABLE;
    attrib[attrCnt].value.type = VAGenericValueTypeInteger;
    attrib[attrCnt++].value.value.i = va_fourcc;
    format = va_fourcc;

    if ((fourcc == MFX_FOURCC_VP8_NV12) ||
        ((MFX_MEMTYPE_VIDEO_MEMORY_ENCODER_TARGET & memType)
            && ((fourcc == MFX_FOURCC_RGB4) || (fourcc == MFX_FOURCC_BGR4))))
    {
        /*
            *  special configuration for NV12 surf allocation for VP8 hybrid encoder and
            *  RGB32 for JPEG is required
            */
        attrib[attrCnt].type = (VASurfaceAttribType)VASurfaceAttribUsageHint;
        attrib[attrCnt].flags = VA_SURFACE_ATTRIB_SETTABLE;
        attrib[attrCnt].value.type = VAGenericValueTypeInteger;
        attrib[attrCnt++].value.value.i = VA_SURFACE_ATTRIB_USAGE_HINT_ENCODER;
    }
    else if (fourcc == MFX_FOURCC_VP8_MBDATA)
    {
        // special configuration for MB data surf allocation for VP8 hybrid encoder is required
        attrib[0].value.value.i = VA_FOURCC_P208;
        format = VA_FOURCC_P208;
    }
    else if (va_fourcc == VA_FOURCC_NV12)
    {
        format = VA_RT_FORMAT_YUV420;
    }

    va_res = m_libva->vaCreateSurfaces(m_dpy,
        format,
        Width, Height,
        surfaces,
        1,
        &attrib[0], attrCnt);

    *vaapiMid->m_surface = surfaces[0];
    vaapiMid->m_fourcc = fourcc;
    *midOut = mid;

    mfx_res = va_to_mfx_status(va_res);

    return mfx_res;
}

mfxStatus vaapiFrameAllocator::AllocImpl(mfxFrameAllocRequest *request, mfxFrameAllocResponse *response)
{
    mfxStatus mfx_res = MFX_ERR_NONE;
    VAStatus  va_res  = VA_STATUS_SUCCESS;
    unsigned int va_fourcc = 0;
    VASurfaceID* surfaces = NULL;
    vaapiMemId *vaapi_mids = NULL, *vaapi_mid = NULL;
    mfxMemId* mids = NULL;
    mfxU32 fourcc = request->Info.FourCC;
    mfxU16 surfaces_num = request->NumFrameSuggested, numAllocated = 0, i = 0;
    bool bCreateSrfSucceeded = false;

    memset(response, 0, sizeof(mfxFrameAllocResponse));

    mfx_res = GetVAFourcc(fourcc, va_fourcc);
    if (MFX_ERR_NONE != mfx_res)
        return mfx_res;

    if (!surfaces_num)
    {
        return MFX_ERR_MEMORY_ALLOC;
    }

    if (MFX_ERR_NONE == mfx_res)
    {
        surfaces = (VASurfaceID*)calloc(surfaces_num, sizeof(VASurfaceID));
        vaapi_mids = (vaapiMemId*)calloc(surfaces_num, sizeof(vaapiMemId));
        mids = (mfxMemId*)calloc(surfaces_num, sizeof(mfxMemId));
        if ((NULL == surfaces) || (NULL == vaapi_mids) || (NULL == mids)) mfx_res = MFX_ERR_MEMORY_ALLOC;
    }
    if (MFX_ERR_NONE == mfx_res)
    {
        if( VA_FOURCC_P208 != va_fourcc )
        {
            unsigned int format;
            VASurfaceAttrib attrib[2];
            int attrCnt = 0;

            attrib[attrCnt].type          = VASurfaceAttribPixelFormat;
            attrib[attrCnt].flags         = VA_SURFACE_ATTRIB_SETTABLE;
            attrib[attrCnt].value.type    = VAGenericValueTypeInteger;
            attrib[attrCnt++].value.value.i = va_fourcc;
            format               = va_fourcc;

            if (fourcc == MFX_FOURCC_VP8_NV12)
            {
                // special configuration for NV12 surf allocation for VP8 hybrid encoder is required
                attrib[attrCnt].type          = (VASurfaceAttribType)VASurfaceAttribUsageHint;
                attrib[attrCnt].flags           = VA_SURFACE_ATTRIB_SETTABLE;
                attrib[attrCnt].value.type      = VAGenericValueTypeInteger;
                attrib[attrCnt++].value.value.i = VA_SURFACE_ATTRIB_USAGE_HINT_ENCODER;
            }
            else if (fourcc == MFX_FOURCC_VP8_MBDATA)
            {
                // special configuration for MB data surf allocation for VP8 hybrid encoder is required
                attrib[0].value.value.i = VA_FOURCC_P208;
                format               = VA_FOURCC_P208;
            }
            else if (va_fourcc == VA_FOURCC_NV12)
            {
                format = VA_RT_FORMAT_YUV420;
            }
            else if ((va_fourcc == VA_FOURCC_UYVY) || (va_fourcc == VA_FOURCC_YUY2))
            {
                format = VA_RT_FORMAT_YUV422;
            }
            else if (fourcc == MFX_FOURCC_A2RGB10)
            {
                format = VA_RT_FORMAT_RGB32_10BPP;
            }
            else if (fourcc == MFX_FOURCC_RGBP)
            {
                format = VA_RT_FORMAT_RGBP;
            }

            va_res = m_libva->vaCreateSurfaces(m_dpy,
                                    format,
                                    request->Info.Width, request->Info.Height,
                                    surfaces,
                                    surfaces_num,
                                    &attrib[0], attrCnt);

            mfx_res = va_to_mfx_status(va_res);
            bCreateSrfSucceeded = (MFX_ERR_NONE == mfx_res);
        }
        else
        {
            VAContextID context_id = request->AllocId;
            int codedbuf_size, codedbuf_num;

            VABufferType codedbuf_type;
            if (fourcc == MFX_FOURCC_VP8_SEGMAP)
            {
                codedbuf_size = request->Info.Width;
                codedbuf_num = request->Info.Height;
                codedbuf_type = VAEncMacroblockMapBufferType;
            }
            else
            {
                int width32 = 32 * ((request->Info.Width + 31) >> 5);
                int height32 = 32 * ((request->Info.Height + 31) >> 5);
                codedbuf_size = static_cast<int>((width32 * height32) * 400LL / (16 * 16));
                codedbuf_num = 1;
                codedbuf_type = VAEncCodedBufferType;
            }

            for (numAllocated = 0; numAllocated < surfaces_num; numAllocated++)
            {
                VABufferID coded_buf;

                va_res = m_libva->vaCreateBuffer(m_dpy,
                                      context_id,
                                      codedbuf_type,
                                      codedbuf_size,
                                      codedbuf_num,
                                      NULL,
                                      &coded_buf);
                mfx_res = va_to_mfx_status(va_res);
                if (MFX_ERR_NONE != mfx_res) break;
                surfaces[numAllocated] = coded_buf;
            }
        }
    }

    if ((MFX_ERR_NONE == mfx_res) &&
        (request->Type & MFX_MEMTYPE_EXPORT_FRAME))
    {
        if (m_export_mode == vaapiAllocatorParams::DONOT_EXPORT) {
            mfx_res = MFX_ERR_UNKNOWN;
        }
        for (i=0; i < surfaces_num; ++i)
        {
            if (m_export_mode & vaapiAllocatorParams::NATIVE_EXPORT_MASK) {
                vaapi_mids[i].m_buffer_info.mem_type = (m_export_mode & vaapiAllocatorParams::PRIME)?
                  VA_SURFACE_ATTRIB_MEM_TYPE_DRM_PRIME: VA_SURFACE_ATTRIB_MEM_TYPE_KERNEL_DRM;
                va_res = m_libva->vaDeriveImage(m_dpy, surfaces[i], &(vaapi_mids[i].m_image));
                mfx_res = va_to_mfx_status(va_res);

                if (MFX_ERR_NONE != mfx_res) break;

                va_res = m_libva->vaAcquireBufferHandle(m_dpy, vaapi_mids[i].m_image.buf, &(vaapi_mids[i].m_buffer_info));

                mfx_res = va_to_mfx_status(va_res);

                if (MFX_ERR_NONE != mfx_res) {
                    m_libva->vaDestroyImage(m_dpy, vaapi_mids[i].m_image.image_id);
                    break;
                }
            }
            if (m_exporter) {
                vaapi_mids[i].m_fourcc = va_fourcc;
                vaapi_mids[i].m_custom = m_exporter->acquire(&vaapi_mids[i]);
                if (!vaapi_mids[i].m_custom) {
                    mfx_res = MFX_ERR_UNKNOWN;
                    break;
                }
            }
        }
    }
    if (MFX_ERR_NONE == mfx_res)
    {
        for (i = 0; i < surfaces_num; ++i)
        {
            vaapi_mid = &(vaapi_mids[i]);
            vaapi_mid->m_fourcc = fourcc;
            vaapi_mid->m_surface = &(surfaces[i]);
            mids[i] = vaapi_mid;
        }
    }
    if (MFX_ERR_NONE == mfx_res)
    {
        response->mids = mids;
        response->NumFrameActual = surfaces_num;
    }
    else // i.e. MFX_ERR_NONE != mfx_res
    {
        response->mids = NULL;
        response->NumFrameActual = 0;
        if (VA_FOURCC_P208 != va_fourcc
            || fourcc == MFX_FOURCC_VP8_MBDATA )
        {
            if (bCreateSrfSucceeded)
                m_libva->vaDestroySurfaces(m_dpy, surfaces, surfaces_num);
        }
        else
        {
            for (i = 0; i < numAllocated; i++)
                m_libva->vaDestroyBuffer(m_dpy, surfaces[i]);
        }
        if (mids)
        {
            free(mids);
            mids = NULL;
        }
        if (vaapi_mids) { free(vaapi_mids); vaapi_mids = NULL; }
        if (surfaces) { free(surfaces); surfaces = NULL; }
    }
    return mfx_res;
}

mfxStatus vaapiFrameAllocator::ReleaseResponse(mfxFrameAllocResponse *response)
{
    vaapiMemId *vaapi_mids = NULL;
    VASurfaceID* surfaces = NULL;
    mfxU32 i = 0;
    bool isBitstreamMemory=false;

    if (!response) return MFX_ERR_NULL_PTR;

    if (response->mids)
    {
        vaapi_mids = (vaapiMemId*)(response->mids[0]);
        mfxU32 mfx_fourcc = ConvertVP8FourccToMfxFourcc(vaapi_mids->m_fourcc);
        isBitstreamMemory = (MFX_FOURCC_P8 == mfx_fourcc)?true:false;
        surfaces = vaapi_mids->m_surface;
        for (i = 0; i < response->NumFrameActual; ++i)
        {
            if (MFX_FOURCC_P8 == vaapi_mids[i].m_fourcc) m_libva->vaDestroyBuffer(m_dpy, surfaces[i]);
            else if (vaapi_mids[i].m_sys_buffer) free(vaapi_mids[i].m_sys_buffer);
            if (m_export_mode != vaapiAllocatorParams::DONOT_EXPORT) {
                if (m_exporter && vaapi_mids[i].m_custom) {
                    m_exporter->release(&vaapi_mids[i], vaapi_mids[i].m_custom);
                }
                if (m_export_mode & vaapiAllocatorParams::NATIVE_EXPORT_MASK) {
                    m_libva->vaReleaseBufferHandle(m_dpy, vaapi_mids[i].m_image.buf);
                    m_libva->vaDestroyImage(m_dpy, vaapi_mids[i].m_image.image_id);
                }
            }
        }
        free(vaapi_mids);
        free(response->mids);
        response->mids = NULL;

        if (!isBitstreamMemory) m_libva->vaDestroySurfaces(m_dpy, surfaces, response->NumFrameActual);
        free(surfaces);
    }
    response->NumFrameActual = 0;
    return MFX_ERR_NONE;
}

mfxStatus vaapiFrameAllocator::LockFrame(mfxMemId mid, mfxFrameData *ptr)
{
    mfxStatus mfx_res = MFX_ERR_NONE;
    VAStatus  va_res  = VA_STATUS_SUCCESS;
    vaapiMemId* vaapi_mid = (vaapiMemId*)mid;
    mfxU8* pBuffer = 0;

    if (!vaapi_mid || !(vaapi_mid->m_surface)) return MFX_ERR_INVALID_HANDLE;

    mfxU32 mfx_fourcc = ConvertVP8FourccToMfxFourcc(vaapi_mid->m_fourcc);

    if (MFX_FOURCC_P8 == mfx_fourcc)   // bitstream processing
    {
        VACodedBufferSegment *coded_buffer_segment;
        if (vaapi_mid->m_fourcc == MFX_FOURCC_VP8_SEGMAP)
            va_res = m_libva->vaMapBuffer(m_dpy, *(vaapi_mid->m_surface), (void **)(&pBuffer));
        else
            va_res = m_libva->vaMapBuffer(m_dpy, *(vaapi_mid->m_surface), (void **)(&coded_buffer_segment));
        mfx_res = va_to_mfx_status(va_res);
        if (MFX_ERR_NONE == mfx_res)
        {
            if (vaapi_mid->m_fourcc == MFX_FOURCC_VP8_SEGMAP)
                ptr->Y = pBuffer;
            else
                ptr->Y = (mfxU8*)coded_buffer_segment->buf;

        }
    }
    else   // Image processing
    {
        va_res = m_libva->vaDeriveImage(m_dpy, *(vaapi_mid->m_surface), &(vaapi_mid->m_image));
        mfx_res = va_to_mfx_status(va_res);

        if (MFX_ERR_NONE == mfx_res)
        {
            va_res = m_libva->vaMapBuffer(m_dpy, vaapi_mid->m_image.buf, (void **)&pBuffer);
            mfx_res = va_to_mfx_status(va_res);
        }
        if (MFX_ERR_NONE == mfx_res)
        {
            switch (vaapi_mid->m_image.format.fourcc)
            {
            case VA_FOURCC_NV12:
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->Y = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->U = pBuffer + vaapi_mid->m_image.offsets[1];
                    ptr->V = ptr->U + 1;
                }
                break;
            case VA_FOURCC_YV12:
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->Y = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->V = pBuffer + vaapi_mid->m_image.offsets[1];
                    ptr->U = pBuffer + vaapi_mid->m_image.offsets[2];
                }
                break;
            case VA_FOURCC_YUY2:
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->Y = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->U = ptr->Y + 1;
                    ptr->V = ptr->Y + 3;
                }
                break;
            case VA_FOURCC_UYVY:
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->U = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->Y = ptr->U + 1;
                    ptr->V = ptr->U + 2;
                }
                break;
#if (MFX_VERSION >= 1028)
            case VA_FOURCC_RGB565:
                if (mfx_fourcc == MFX_FOURCC_RGB565)
                {
                    ptr->B = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->G = ptr->B;
                    ptr->R = ptr->B;
                }
                else return MFX_ERR_LOCK_MEMORY;
                break;
#endif
            case VA_FOURCC_ARGB:
                if (mfx_fourcc == MFX_FOURCC_RGB4)
                {
                    ptr->B = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->G = ptr->B + 1;
                    ptr->R = ptr->B + 2;
                    ptr->A = ptr->B + 3;
                }
                else return MFX_ERR_LOCK_MEMORY;
                break;
#ifndef ANDROID
            case VA_FOURCC_A2R10G10B10:
                if (mfx_fourcc == MFX_FOURCC_A2RGB10)
                {
                    ptr->B = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->G = ptr->B;
                    ptr->R = ptr->B;
                    ptr->A = ptr->B;
                }
                else return MFX_ERR_LOCK_MEMORY;
                break;
#endif
            case VA_FOURCC_ABGR:
                if (mfx_fourcc == MFX_FOURCC_BGR4)
                {
                    ptr->R = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->G = pBuffer + vaapi_mid->m_image.offsets[1];
                    ptr->B = pBuffer + vaapi_mid->m_image.offsets[2];
                    ptr->A = ptr->R + 3;
                }
                else return MFX_ERR_LOCK_MEMORY;
                break;
            case VA_FOURCC_RGBP:
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->B = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->G = pBuffer + vaapi_mid->m_image.offsets[1];
                    ptr->R = pBuffer + vaapi_mid->m_image.offsets[2];
                }
                break;
            case VA_FOURCC_P208:
                if (mfx_fourcc == MFX_FOURCC_NV12)
                {
                    ptr->Y = pBuffer + vaapi_mid->m_image.offsets[0];
                }
                else return MFX_ERR_LOCK_MEMORY;
                break;
            case VA_FOURCC_P010:
#if (MFX_VERSION >= 1031)
            case VA_FOURCC_P016:
#endif
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->Y16 = (mfxU16 *) (pBuffer + vaapi_mid->m_image.offsets[0]);
                    ptr->U16 = (mfxU16 *) (pBuffer + vaapi_mid->m_image.offsets[1]);
                    ptr->V16 = ptr->U16 + 1;
                }
                break;
            case VA_FOURCC_AYUV:
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->V = pBuffer + vaapi_mid->m_image.offsets[0];
                    ptr->U = ptr->V + 1;
                    ptr->Y = ptr->V + 2;
                    ptr->A = ptr->V + 3;
                }
                break;
#if (MFX_VERSION >= 1027)
            case VA_FOURCC_Y210:
#if (MFX_VERSION >= 1031)
            case VA_FOURCC_Y216:
#endif
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->Y16 = (mfxU16 *) (pBuffer + vaapi_mid->m_image.offsets[0]);
                    ptr->U16 = ptr->Y16 + 1;
                    ptr->V16 = ptr->Y16 + 3;
                }
                break;
            case VA_FOURCC_Y410:
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->Y410 = (mfxY410 *)(pBuffer + vaapi_mid->m_image.offsets[0]);
                    ptr->Y = 0;
                    ptr->V = 0;
                    ptr->A = 0;
                }
                break;
#endif
#if (MFX_VERSION >= 1031)
            case VA_FOURCC_Y416:
                if (mfx_fourcc != vaapi_mid->m_image.format.fourcc) return MFX_ERR_LOCK_MEMORY;

                {
                    ptr->U16 = (mfxU16 *) (pBuffer + vaapi_mid->m_image.offsets[0]);
                    ptr->Y16 = ptr->U16 + 1;
                    ptr->V16 = ptr->Y16 + 1;
                    ptr->A   = (mfxU8 *)(ptr->V16 + 1);
                }
                break;
#endif
            default:
                return MFX_ERR_LOCK_MEMORY;
            }
        }

        ptr->PitchHigh = (mfxU16)(vaapi_mid->m_image.pitches[0] / (1 << 16));
        ptr->PitchLow  = (mfxU16)(vaapi_mid->m_image.pitches[0] % (1 << 16));
    }
    return mfx_res;
}

mfxStatus vaapiFrameAllocator::UnlockFrame(mfxMemId mid, mfxFrameData *ptr)
{
    vaapiMemId* vaapi_mid = (vaapiMemId*)mid;

    if (!vaapi_mid || !(vaapi_mid->m_surface)) return MFX_ERR_INVALID_HANDLE;

    mfxU32 mfx_fourcc = ConvertVP8FourccToMfxFourcc(vaapi_mid->m_fourcc);

    if (MFX_FOURCC_P8 == mfx_fourcc)   // bitstream processing
    {
        m_libva->vaUnmapBuffer(m_dpy, *(vaapi_mid->m_surface));
    }
    else  // Image processing
    {
        m_libva->vaUnmapBuffer(m_dpy, vaapi_mid->m_image.buf);
        m_libva->vaDestroyImage(m_dpy, vaapi_mid->m_image.image_id);

        if (NULL != ptr)
        {
            ptr->PitchLow  = 0;
            ptr->PitchHigh = 0;
            ptr->Y     = NULL;
            ptr->U     = NULL;
            ptr->V     = NULL;
            ptr->A     = NULL;
        }
    }
    return MFX_ERR_NONE;
}

mfxStatus vaapiFrameAllocator::GetFrameHDL(mfxMemId mid, mfxHDL *handle)
{
    vaapiMemId* vaapi_mid = (vaapiMemId*)mid;

    if (!handle || !vaapi_mid || !(vaapi_mid->m_surface)) return MFX_ERR_INVALID_HANDLE;

    *handle = vaapi_mid->m_surface; //VASurfaceID* <-> mfxHDL
    return MFX_ERR_NONE;
}

#endif // #if defined(LIBVA_SUPPORT)
