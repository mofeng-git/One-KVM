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

#include "mfx_samples_config.h"

#include <stdlib.h>

#include <mfx_buffering.h>

CBuffering::CBuffering():
    m_SurfacesNumber(0),
    m_OutputSurfacesNumber(0),
    m_pSurfaces(NULL),
    m_pVppSurfaces(NULL),
    m_FreeSurfacesPool(m_Mutex),
    m_FreeVppSurfacesPool(m_Mutex),
    m_UsedSurfacesPool(m_Mutex),
    m_UsedVppSurfacesPool(m_Mutex),
    m_pFreeOutputSurfaces(NULL),
    m_OutputSurfacesPool(m_Mutex),
    m_DeliveredSurfacesPool(m_Mutex)
{
}

CBuffering::~CBuffering()
{
}

mfxStatus
CBuffering::AllocBuffers(mfxU32 SurfaceNumber)
{
    if (!SurfaceNumber) return MFX_ERR_MEMORY_ALLOC;

    if (!m_OutputSurfacesNumber) { // true - if Vpp isn't enabled
        m_OutputSurfacesNumber = SurfaceNumber;
    }
    m_SurfacesNumber = SurfaceNumber;

    m_pSurfaces = (msdkFrameSurface*)calloc(m_SurfacesNumber, sizeof(msdkFrameSurface));
    if (!m_pSurfaces) return MFX_ERR_MEMORY_ALLOC;

    msdkOutputSurface* p = NULL;
    msdkOutputSurface* tail = NULL;

    m_pFreeOutputSurfaces = (msdkOutputSurface*)calloc(1, sizeof(msdkOutputSurface));
    if (!m_pFreeOutputSurfaces) return MFX_ERR_MEMORY_ALLOC;

    tail = m_pFreeOutputSurfaces;

    for (mfxU32 i = 1; i < m_OutputSurfacesNumber; ++i) {
        p = (msdkOutputSurface*)calloc(1, sizeof(msdkOutputSurface));
        if (!p) return MFX_ERR_MEMORY_ALLOC;
        tail->next = p;
        tail = p;
    }

    ResetBuffers();
    return MFX_ERR_NONE;
}

mfxStatus
CBuffering::AllocVppBuffers(mfxU32 VppSurfaceNumber)
{
    m_OutputSurfacesNumber = VppSurfaceNumber;
    m_pVppSurfaces = (msdkFrameSurface*)calloc(m_OutputSurfacesNumber, sizeof(msdkFrameSurface));
    if (!m_pVppSurfaces) return MFX_ERR_MEMORY_ALLOC;

    ResetVppBuffers();
    return MFX_ERR_NONE;
}

void
CBuffering::AllocOutputBuffer()
{
    std::lock_guard<std::mutex> lock(m_Mutex);

    m_pFreeOutputSurfaces = (msdkOutputSurface*)calloc(1, sizeof(msdkOutputSurface));
}

static void
FreeList(msdkOutputSurface*& head) {
    msdkOutputSurface* next;
    while (head) {
        next = head->next;
        free(head);
        head = next;
    }
}

void
CBuffering::FreeBuffers()
{
    if (m_pSurfaces) {
        free(m_pSurfaces);
        m_pSurfaces = NULL;
    }

    if (m_pVppSurfaces) {
        free(m_pVppSurfaces);
        m_pVppSurfaces = NULL;
    }

    FreeList(m_pFreeOutputSurfaces);
    FreeList(m_OutputSurfacesPool.m_pSurfacesHead);
    FreeList(m_DeliveredSurfacesPool.m_pSurfacesHead);

    m_UsedSurfacesPool.m_pSurfacesHead = NULL;
    m_UsedSurfacesPool.m_pSurfacesTail = NULL;
    m_UsedVppSurfacesPool.m_pSurfacesHead = NULL;
    m_UsedVppSurfacesPool.m_pSurfacesTail = NULL;
    m_OutputSurfacesPool.m_pSurfacesHead = NULL;
    m_OutputSurfacesPool.m_pSurfacesTail = NULL;

    m_FreeSurfacesPool.m_pSurfaces = NULL;
    m_FreeVppSurfacesPool.m_pSurfaces = NULL;
}

void
CBuffering::ResetBuffers()
{
    mfxU32 i;
    msdkFrameSurface* pFreeSurf = m_FreeSurfacesPool.m_pSurfaces = m_pSurfaces;

    for (i = 0; i < m_SurfacesNumber; ++i) {
        if (i < (m_SurfacesNumber-1)) {
            pFreeSurf[i].next = &(pFreeSurf[i+1]);
            pFreeSurf[i+1].prev = &(pFreeSurf[i]);
        }
    }
}

void
CBuffering::ResetVppBuffers()
{
    mfxU32 i;
    msdkFrameSurface* pFreeVppSurf = m_FreeVppSurfacesPool.m_pSurfaces = m_pVppSurfaces;

    for (i = 0; i < m_OutputSurfacesNumber; ++i) {
        if (i < (m_OutputSurfacesNumber-1)) {
            pFreeVppSurf[i].next = &(pFreeVppSurf[i+1]);
            pFreeVppSurf[i+1].prev = &(pFreeVppSurf[i]);
        }
    }
}

void
CBuffering::SyncFrameSurfaces()
{
    std::lock_guard<std::mutex> lock(m_Mutex);
    msdkFrameSurface *next = NULL;
    msdkFrameSurface *cur = m_UsedSurfacesPool.m_pSurfacesHead;

    while (cur) {
        if (cur->frame.Data.Locked || cur->render_lock) {
            // frame is still locked: just moving to the next one
            cur = cur->next;
        } else {
            // frame was unlocked: moving it to the free surfaces array
            m_UsedSurfacesPool.DetachSurfaceUnsafe(cur);
            m_FreeSurfacesPool.AddSurfaceUnsafe(cur);

            cur = next;
        }
    }
}

void
CBuffering::SyncVppFrameSurfaces()
{
    std::lock_guard<std::mutex> lock(m_Mutex);
    msdkFrameSurface *next = NULL;
    msdkFrameSurface *cur = m_UsedVppSurfacesPool.m_pSurfacesHead;

    while (cur) {
        if (cur->frame.Data.Locked || cur->render_lock) {
            // frame is still locked: just moving to the next one
            cur = cur->next;
        } else {
            // frame was unlocked: moving it to the free surfaces array
            m_UsedVppSurfacesPool.DetachSurfaceUnsafe(cur);
            m_FreeVppSurfacesPool.AddSurfaceUnsafe(cur);

            cur = next;
        }
    }
}
