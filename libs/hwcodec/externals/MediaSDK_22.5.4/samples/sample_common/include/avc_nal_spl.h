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
#ifndef __AVC_NAL_SPL_H
#define __AVC_NAL_SPL_H

#include <vector>
#include "mfxstructures.h"

namespace ProtectedLibrary
{

class BytesSwapper
{
public:
    static void SwapMemory(mfxU8 *pDestination, mfxU32 &nDstSize, mfxU8 *pSource, mfxU32 nSrcSize);
};


class StartCodeIterator
{
public:

    StartCodeIterator();

    void Reset();

    mfxI32 Init(mfxBitstream * source);

    void SetSuggestedSize(mfxU32 size);

    mfxI32 CheckNalUnitType(mfxBitstream * source);

    mfxI32 GetNALUnit(mfxBitstream * source, mfxBitstream * destination);

    mfxI32 EndOfStream(mfxBitstream * destination);

private:
    std::vector<mfxU8>  m_prev;
    mfxU32   m_code;
    mfxU64   m_pts;

    mfxU8 * m_pSource;
    mfxU32  m_nSourceSize;

    mfxU8 * m_pSourceBase;
    mfxU32  m_nSourceBaseSize;

    mfxU32  m_suggestedSize;

    mfxI32 FindStartCode(mfxU8 * (&pb), mfxU32 & size, mfxI32 & startCodeSize);
};

class NALUnitSplitter
{
public:

    NALUnitSplitter();

    virtual ~NALUnitSplitter();

    virtual void Init();
    virtual void Release();

    virtual mfxI32 CheckNalUnitType(mfxBitstream * source);
    virtual mfxI32 GetNalUnits(mfxBitstream * source, mfxBitstream * &destination);

    virtual void Reset();

    virtual void SetSuggestedSize(mfxU32 size)
    {
        m_pStartCodeIter.SetSuggestedSize(size);
    }

protected:

    StartCodeIterator m_pStartCodeIter;

    mfxBitstream m_bitstream;
};

void SwapMemoryAndRemovePreventingBytes(mfxU8 *pDestination, mfxU32 &nDstSize, mfxU8 *pSource, mfxU32 nSrcSize);

} //namespace ProtectedLibrary

#endif // __AVC_NAL_SPL_H
