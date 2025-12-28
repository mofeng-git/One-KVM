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

#pragma once

#include "mfxstructures.h"
#include "sample_utils.h"

/*
    Rationale: locks allocator if necessary to get RAW pointers, unlock it at the end
*/
class SurfaceAutoLock : private no_copy
{
public:
    SurfaceAutoLock(mfxFrameAllocator & alloc, mfxFrameSurface1 &srf)
        : m_alloc(alloc) , m_srf(srf), m_lockRes(MFX_ERR_NONE), m_bLocked() {
        LockFrame();
    }
    operator mfxStatus () {
        return m_lockRes;
    }
    ~SurfaceAutoLock() {
        UnlockFrame();
    }

protected:

    mfxFrameAllocator & m_alloc;
    mfxFrameSurface1 & m_srf;
    mfxStatus m_lockRes;
    bool m_bLocked;

    void LockFrame()
    {
        //no allocator used, no need to do lock
        if (m_srf.Data.Y != 0)
            return ;
        //lock required
        m_lockRes = m_alloc.Lock(m_alloc.pthis, m_srf.Data.MemId, &m_srf.Data);
        if (m_lockRes == MFX_ERR_NONE) {
            m_bLocked = true;
        }
    }

    void UnlockFrame()
    {
        if (m_lockRes != MFX_ERR_NONE || !m_bLocked) {
            return;
        }
        //unlock required
        m_alloc.Unlock(m_alloc.pthis, m_srf.Data.MemId, &m_srf.Data);
    }
};