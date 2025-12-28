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

#ifndef __VPP_EX_H__
#define __VPP_EX_H__

#include "sample_utils.h"
#include "mfxvideo++.h"
#include <vector>

/* #define USE_VPP_EX */

class MFXVideoVPPEx : public MFXVideoVPP
{

public:
    MFXVideoVPPEx(mfxSession session);

#if defined USE_VPP_EX

    mfxStatus QueryIOSurf(mfxVideoParam *par, mfxFrameAllocRequest request[2]);
    mfxStatus Query(mfxVideoParam *in, mfxVideoParam *out);
    mfxStatus Init(mfxVideoParam *par);
    mfxStatus RunFrameVPPAsync(mfxFrameSurface1 *in, mfxFrameSurface1 *out, mfxExtVppAuxData *aux, mfxSyncPoint *syncp);
    mfxStatus GetVideoParam(mfxVideoParam *par);
    mfxStatus Close(void);

protected:

    std::vector<mfxFrameSurface1*>  m_LockedSurfacesList;
    mfxVideoParam                   m_VideoParams;

    mfxU64                          m_nCurrentPTS;

    mfxU64                          m_nIncreaseTime;
    mfxU64                          m_nArraySize;
    mfxU64                          m_nInputTimeStamp;

#endif

};

#endif //__VPP_EX_H__
