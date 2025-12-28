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

#ifndef __MFX_BUFFERING_H__
#define __MFX_BUFFERING_H__

#include <stdio.h>
#include <mutex>

#include "mfxstructures.h"

#include "vm/strings_defs.h"
#include "vm/thread_defs.h"
#include "vm/time_defs.h"
#include "vm/atomic_defs.h"

struct msdkFrameSurface
{
    mfxFrameSurface1 frame; // NOTE: this _should_ be the first item (see CBuffering::FindUsedSurface())
    msdk_tick submit;       // tick when frame was submitted for processing
    mfxU16 render_lock;     // signifies that frame is locked for rendering
    msdkFrameSurface* prev;
    msdkFrameSurface* next;
};

struct msdkOutputSurface
{
    msdkFrameSurface* surface;
    mfxSyncPoint syncp;
    msdkOutputSurface* next;
};

/** \brief Debug purpose macro to terminate execution if buggy situation happenned.
 *
 * Use this macro to check impossible, buggy condition which should not occur under
 * normal circumstances. Macro should be used where check in release mode is not
 * desirable and atually needed.
 */
#define MSDK_SELF_CHECK(C)

class CBuffering;

// LIFO list of frame surfaces
class msdkFreeSurfacesPool
{
    friend class CBuffering;
public:
    msdkFreeSurfacesPool(std::mutex & mutex):
        m_pSurfaces(NULL),
        m_rMutex(mutex) {}

    ~msdkFreeSurfacesPool() {
        m_pSurfaces = NULL;
    }
    /** \brief The function adds free surface to the free surfaces array.
     *
     * @note That's caller responsibility to pass valid surface.
     * @note We always add and get free surface from the array head. In case not all surfaces
     * will be actually used we have good chance to avoid actual allocation of the surface memory.
     */
    inline void AddSurface(msdkFrameSurface* surface) {
        std::lock_guard<std::mutex> lock(m_rMutex);
        AddSurfaceUnsafe(surface);
    }
    /** \brief The function gets the next free surface from the free surfaces array.
     *
     * @note Surface is detached from the free surfaces array.
     */
    inline msdkFrameSurface* GetSurface() {
        std::lock_guard<std::mutex> lock(m_rMutex);
        return GetSurfaceUnsafe();
    }

private:
    inline void AddSurfaceUnsafe(msdkFrameSurface* surface) {
        msdkFrameSurface* head;

        MSDK_SELF_CHECK(surface);
        MSDK_SELF_CHECK(!surface->prev);
        MSDK_SELF_CHECK(!surface->next);

        head = m_pSurfaces;
        m_pSurfaces = surface;
        m_pSurfaces->next = head;
    }
    inline msdkFrameSurface* GetSurfaceUnsafe() {

        msdkFrameSurface* surface = NULL;

        if (m_pSurfaces) {
            surface = m_pSurfaces;
            m_pSurfaces = m_pSurfaces->next;
            surface->prev = surface->next = NULL;
            MSDK_SELF_CHECK(!surface->prev);
            MSDK_SELF_CHECK(!surface->next);
        }
        return surface;
    }

protected:
    msdkFrameSurface* m_pSurfaces;
    std::mutex &      m_rMutex;

private:
    msdkFreeSurfacesPool(const msdkFreeSurfacesPool&);
    void operator=(const msdkFreeSurfacesPool&);
};

// random access, predicted as FIFO
class msdkUsedSurfacesPool
{
    friend class CBuffering;
public:
    msdkUsedSurfacesPool(std::mutex & mutex):
        m_pSurfacesHead(NULL),
        m_pSurfacesTail(NULL),
        m_rMutex(mutex) {}

    ~msdkUsedSurfacesPool() {
        m_pSurfacesHead = NULL;
        m_pSurfacesTail = NULL;
    }

    /** \brief The function adds surface to the used surfaces array (m_pUsedSurfaces).
     *
     * @note That's caller responsibility to pass valid surface.
     * @note We can't actually know which surface will be returned by the decoder or unlocked. However,
     * we can make prediction that it will be the oldest surface. Thus, here the function adds new
     * surface (youngest) to the tail of the least. Check operations for the list will run starting from
     * head.
     */
    inline void AddSurface(msdkFrameSurface* surface) {
        std::lock_guard<std::mutex> lock(m_rMutex);
        AddSurfaceUnsafe(surface);
    }

    /** \brief The function detaches surface from the used surfaces array.
     *
     * @note That's caller responsibility to pass valid surface.
     */

    inline void DetachSurface(msdkFrameSurface* surface) {
        std::lock_guard<std::mutex> lock(m_rMutex);
        DetachSurfaceUnsafe(surface);
    }

private:
    inline void DetachSurfaceUnsafe(msdkFrameSurface* surface)
    {
        MSDK_SELF_CHECK(surface);

        msdkFrameSurface *prev = surface->prev;
        msdkFrameSurface *next = surface->next;

        if (prev) {
            prev->next = next;
        }
        else {
            MSDK_SELF_CHECK(surface == m_pSurfacesHead);
            m_pSurfacesHead = next;
        }
        if (next) {
            next->prev = prev;
        } else {
            MSDK_SELF_CHECK(surface == m_pSurfacesTail);
            m_pSurfacesTail = prev;
        }

        surface->prev = surface->next = NULL;
        MSDK_SELF_CHECK(!surface->prev);
        MSDK_SELF_CHECK(!surface->next);
    }
    inline void AddSurfaceUnsafe(msdkFrameSurface* surface)
    {
        MSDK_SELF_CHECK(surface);
        MSDK_SELF_CHECK(!surface->prev);
        MSDK_SELF_CHECK(!surface->next);

        surface->prev = m_pSurfacesTail;
        surface->next = NULL;
        if (m_pSurfacesTail) {
            m_pSurfacesTail->next = surface;
            m_pSurfacesTail = m_pSurfacesTail->next;
        } else {
            m_pSurfacesHead = m_pSurfacesTail = surface;
        }
    }

protected:
    msdkFrameSurface* m_pSurfacesHead; // oldest surface
    msdkFrameSurface* m_pSurfacesTail; // youngest surface
    std::mutex & m_rMutex;

private:
    msdkUsedSurfacesPool(const msdkUsedSurfacesPool&);
    void operator=(const msdkUsedSurfacesPool&);
};

// FIFO list of surfaces
class msdkOutputSurfacesPool
{
    friend class CBuffering;
public:
    msdkOutputSurfacesPool(std::mutex & mutex):
        m_pSurfacesHead(NULL),
        m_pSurfacesTail(NULL),
        m_SurfacesCount(0),
        m_rMutex(mutex) {}

    ~msdkOutputSurfacesPool() {
        m_pSurfacesHead = NULL;
        m_pSurfacesTail = NULL;
    }

    inline void AddSurface(msdkOutputSurface* surface) {
        std::lock_guard<std::mutex> lock(m_rMutex);
        AddSurfaceUnsafe(surface);
    }
    inline msdkOutputSurface* GetSurface() {
        std::lock_guard<std::mutex> lock(m_rMutex);
        return GetSurfaceUnsafe();
    }

    inline mfxU32 GetSurfaceCount() {
        return m_SurfacesCount;
    }
private:
    inline void AddSurfaceUnsafe(msdkOutputSurface* surface)
    {
        MSDK_SELF_CHECK(surface);
        MSDK_SELF_CHECK(!surface->next);
        surface->next = NULL;

        if (m_pSurfacesTail) {
            m_pSurfacesTail->next = surface;
            m_pSurfacesTail = m_pSurfacesTail->next;
        } else {
            m_pSurfacesHead = m_pSurfacesTail = surface;
        }
        ++m_SurfacesCount;
    }
    inline msdkOutputSurface* GetSurfaceUnsafe()
    {
        msdkOutputSurface* surface = NULL;

        if (m_pSurfacesHead) {
            surface = m_pSurfacesHead;
            m_pSurfacesHead = m_pSurfacesHead->next;
            if (!m_pSurfacesHead) {
                // there was only one surface in the array...
                m_pSurfacesTail = NULL;
            }
            --m_SurfacesCount;
            surface->next = NULL;
            MSDK_SELF_CHECK(!surface->next);
        }
        return surface;
    }

protected:
    msdkOutputSurface*      m_pSurfacesHead; // oldest surface
    msdkOutputSurface*      m_pSurfacesTail; // youngest surface
    mfxU32                  m_SurfacesCount;
    std::mutex &            m_rMutex;

private:
    msdkOutputSurfacesPool(const msdkOutputSurfacesPool&);
    void operator=(const msdkOutputSurfacesPool&);
};

/** \brief Helper class defining optimal buffering operations for the Media SDK decoder.
 */
class CBuffering
{
public:
    CBuffering();
    virtual ~CBuffering();

protected: // functions
    mfxStatus AllocBuffers(mfxU32 SurfaceNumber);
    mfxStatus AllocVppBuffers(mfxU32 VppSurfaceNumber);
    void AllocOutputBuffer();
    void FreeBuffers();
    void ResetBuffers();
    void ResetVppBuffers();

    /** \brief The function syncs arrays of free and used surfaces.
     *
     * If Media SDK used surface for internal needs and unlocked it, the function moves such a surface
     * back to the free surfaces array.
     */
    void SyncFrameSurfaces();
    void SyncVppFrameSurfaces();

    /** \brief Returns surface which corresponds to the given one in Media SDK format (mfxFrameSurface1).
     *
     * @note This function will not detach the surface from the array, perform this explicitly.
     */
    inline msdkFrameSurface* FindUsedSurface(mfxFrameSurface1* frame)
    {
        return (msdkFrameSurface*)(frame);
    }

    inline void AddFreeOutputSurfaceUnsafe(msdkOutputSurface* surface)
    {
        msdkOutputSurface* head = m_pFreeOutputSurfaces;

        MSDK_SELF_CHECK(surface);
        MSDK_SELF_CHECK(!surface->next);
        m_pFreeOutputSurfaces = surface;
        m_pFreeOutputSurfaces->next = head;
    }
    inline void AddFreeOutputSurface(msdkOutputSurface* surface) {
        std::lock_guard<std::mutex> lock(m_Mutex);
        AddFreeOutputSurfaceUnsafe(surface);
    }

    inline msdkOutputSurface* GetFreeOutputSurfaceUnsafe(std::unique_lock<std::mutex> & lock)
    {
        msdkOutputSurface* surface = NULL;

        if (!m_pFreeOutputSurfaces) {
            lock.unlock();
            AllocOutputBuffer();
            lock.lock();
        }
        if (m_pFreeOutputSurfaces) {
            surface = m_pFreeOutputSurfaces;
            m_pFreeOutputSurfaces = m_pFreeOutputSurfaces->next;
            surface->next = NULL;
            MSDK_SELF_CHECK(!surface->next);
        }
        return surface;
    }
    inline msdkOutputSurface* GetFreeOutputSurface() {
        std::unique_lock<std::mutex> lock(m_Mutex);
        return GetFreeOutputSurfaceUnsafe(lock);
    }

    /** \brief Function returns surface data to the corresponding buffers.
     */
    inline void ReturnSurfaceToBuffers(msdkOutputSurface* output_surface)
    {
        MSDK_SELF_CHECK(output_surface);
        MSDK_SELF_CHECK(output_surface->surface);
        MSDK_SELF_CHECK(output_surface->syncp);

        msdk_atomic_dec16(&(output_surface->surface->render_lock));

        output_surface->surface = NULL;
        output_surface->syncp = NULL;

        AddFreeOutputSurface(output_surface);
    }

protected: // variables
    mfxU32                  m_SurfacesNumber;
    mfxU32                  m_OutputSurfacesNumber;
    msdkFrameSurface*       m_pSurfaces;
    msdkFrameSurface*       m_pVppSurfaces;
    std::mutex              m_Mutex;

    // LIFO list of frame surfaces
    msdkFreeSurfacesPool    m_FreeSurfacesPool;
    msdkFreeSurfacesPool    m_FreeVppSurfacesPool;

    // random access, predicted as FIFO
    msdkUsedSurfacesPool    m_UsedSurfacesPool;
    msdkUsedSurfacesPool    m_UsedVppSurfacesPool;

    // LIFO list of output surfaces
    msdkOutputSurface*      m_pFreeOutputSurfaces;

    // FIFO list of surfaces
    msdkOutputSurfacesPool  m_OutputSurfacesPool;
    msdkOutputSurfacesPool  m_DeliveredSurfacesPool;

private:
    CBuffering(const CBuffering&);
    void operator=(const CBuffering&);
};

#endif // __MFX_BUFFERING_H__
