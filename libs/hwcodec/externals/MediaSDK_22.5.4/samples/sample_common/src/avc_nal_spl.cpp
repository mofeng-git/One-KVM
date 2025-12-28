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

#include "sample_defs.h"
#include "avc_structures.h"
#include "avc_nal_spl.h"

namespace ProtectedLibrary
{

static const mfxU32 MFX_TIME_STAMP_FREQUENCY = 90000; // will go to mfxdefs.h
static const mfxU64 MFX_TIME_STAMP_INVALID = (mfxU64)-1; // will go to mfxdefs.h

inline
mfxF64 GetUmcTimeStamp(mfxU64 ts)
{
    return ts == MFX_TIME_STAMP_INVALID ? -1.0 : ts / (mfxF64)MFX_TIME_STAMP_FREQUENCY;
}

inline
mfxU64 GetMfxTimeStamp(mfxF64 ts)
{
    return ts < 0.0 ? MFX_TIME_STAMP_INVALID : (mfxU64)(ts * MFX_TIME_STAMP_FREQUENCY + .5);
}

enum
{
    AVC_NAL_UNITTYPE_BITS_MASK  = 0x1f
};


inline bool IsHeaderCode(mfxI32 iCode)
{
    return (NAL_UT_SPS == (iCode & AVC_NAL_UNITTYPE_BITS_MASK)) ||
           (NAL_UT_SPS_EX == (iCode & AVC_NAL_UNITTYPE_BITS_MASK)) ||
           (NAL_UT_PPS == (iCode & AVC_NAL_UNITTYPE_BITS_MASK));
}

inline bool IsVLCCode(mfxI32 iCode)
{
    return ((NAL_UT_SLICE <= (iCode & AVC_NAL_UNITTYPE_BITS_MASK)) &&
           (NAL_UT_IDR_SLICE >= (iCode & AVC_NAL_UNITTYPE_BITS_MASK))) ||
           (NAL_UT_AUXILIARY == (iCode & AVC_NAL_UNITTYPE_BITS_MASK));
}

static mfxI32 FindStartCode(mfxU8 * (&pb), mfxU32 &nSize)
{
    // there is no data
    if (nSize < 4)
        return 0;

    // find start code
    while ((4 <= nSize) && ((0 != pb[0]) ||
                            (0 != pb[1]) ||
                            (1 != pb[2])))
    {
        pb += 1;
        nSize -= 1;
    }

    if (4 <= nSize)
        return ((pb[0] << 24) | (pb[1] << 16) | (pb[2] << 8) | (pb[3]));

    return 0;

}

mfxStatus MoveBitstream(mfxBitstream * source, mfxI32 moveSize)
{
    if (!source)
        return MFX_ERR_NULL_PTR;

    if (moveSize < 0 && (mfxI32)source->DataOffset + moveSize < 0)
        return MFX_ERR_UNDEFINED_BEHAVIOR;

    if (moveSize > 0 && source->DataLength < (mfxU32)moveSize)
        return MFX_ERR_UNDEFINED_BEHAVIOR;

    source->DataOffset += moveSize;
    source->DataLength -= moveSize;

    return MFX_ERR_NONE;
}

StartCodeIterator::StartCodeIterator()
    : m_code(0)
    , m_pts(MFX_TIME_STAMP_INVALID)
    , m_pSource(0)
    , m_nSourceSize(0)
    , m_pSourceBase(0)
    , m_nSourceBaseSize(0)
    , m_suggestedSize(10 * 1024)
{
    Reset();
}

void StartCodeIterator::Reset()
{
    m_code = 0;
    m_pts = MFX_TIME_STAMP_INVALID;
    m_prev.clear();
}

mfxI32 StartCodeIterator::Init(mfxBitstream * source)
{
    Reset();

    m_pSourceBase = m_pSource = source->Data + source->DataOffset;
    m_nSourceBaseSize = m_nSourceSize = source->DataLength;

    mfxI32 iCode = ProtectedLibrary::FindStartCode(m_pSource, m_nSourceSize);
    return iCode;
}

void StartCodeIterator::SetSuggestedSize(mfxU32 size)
{
    if (size > m_suggestedSize)
        m_suggestedSize = size;
}

mfxI32 StartCodeIterator::CheckNalUnitType(mfxBitstream * source)
{
    if (!source)
        return 0;

    if (!m_code)
        m_prev.clear();

    mfxU8 * src = source->Data + source->DataOffset;
    mfxU32  size = source->DataLength;

    mfxI32 startCodeSize;
    mfxI32 iCodeNext = FindStartCode(src, size, startCodeSize);
    return iCodeNext;
}

mfxI32 StartCodeIterator::GetNALUnit(mfxBitstream * src, mfxBitstream * dst)
{
    if (!src)
        return EndOfStream(dst);

    if (!m_code)
        m_prev.clear();

    mfxU8 * source = src->Data + src->DataOffset;
    mfxU32 size = src->DataLength;

    if (!size)
        return 0;

    mfxI32 startCodeSize;

    mfxI32 iCodeNext = FindStartCode(source, size, startCodeSize);

    if (m_prev.size())
    {
        if (!iCodeNext)
        {
            size_t sz = source - (src->Data + src->DataOffset);
            if (m_prev.size() + sz >  m_suggestedSize)
            {
                m_prev.clear();
                sz = std::min<size_t>(sz, m_suggestedSize);
            }

            m_prev.insert(m_prev.end(), src->Data + src->DataOffset, src->Data + src->DataOffset + sz);
            MoveBitstream(src, (mfxI32)sz);
            return 0;
        }

        source -= startCodeSize;
        m_prev.insert(m_prev.end(), src->Data + src->DataOffset, source);
        MoveBitstream(src, (mfxI32)(source - (src->Data + src->DataOffset)));

        dst->Data = &(m_prev[0]);
        dst->DataLength = (mfxU32)m_prev.size();
        dst->DataOffset = 0;
        dst->TimeStamp = m_pts;
        mfxI32 code = m_code;
        m_code = 0;
        m_pts = MFX_TIME_STAMP_INVALID;
        return code;
    }

    if (!iCodeNext)
    {
        MoveBitstream(src, (mfxI32)(source - (src->Data + src->DataOffset)));
        return 0;
    }

    m_pts = src->TimeStamp;
    m_code = iCodeNext;

    // move before start code
    MoveBitstream(src, (mfxI32)(source - (src->Data + src->DataOffset) - startCodeSize));

    mfxI32 startCodeSize1;
    iCodeNext = FindStartCode(source, size, startCodeSize1);

    MoveBitstream(src, startCodeSize);

    if (!iCodeNext && (src->DataFlag & MFX_BITSTREAM_COMPLETE_FRAME))
    {
        iCodeNext = 1;
        startCodeSize1 = 0;
    }

    if (!iCodeNext)
    {
        if (m_prev.size()) // assertion: it should be
            return 0;

        size_t sz = source - (src->Data + src->DataOffset);
        if (sz > m_suggestedSize)
        {
            sz = m_suggestedSize;
        }

        m_prev.insert(m_prev.end(), src->Data + src->DataOffset, src->Data + src->DataOffset + sz);
        MoveBitstream(src, (mfxI32)sz);
        return 0;
    }

    // fill
    size_t nal_size = source - (src->Data + src->DataOffset) - startCodeSize1;

    dst->Data = src->Data + src->DataOffset;
    dst->DataLength = (mfxU32)nal_size;
    dst->DataOffset = 0;
    dst->TimeStamp = m_pts;

    MoveBitstream(src, (mfxI32)nal_size);

    mfxI32 code = m_code;
    m_code = 0;

    m_pts = MFX_TIME_STAMP_INVALID;
    return code;
}

mfxI32 StartCodeIterator::EndOfStream(mfxBitstream * dst)
{
    if (!m_code)
    {
        m_prev.clear();
        return 0;
    }

    if (m_prev.size())
    {
        dst->Data = &(m_prev[0]);
        dst->DataLength = (mfxU32)m_prev.size();
        dst->DataOffset = 0;
        dst->TimeStamp = m_pts;
        mfxI32 code = m_code;
        m_code = 0;
        m_pts = MFX_TIME_STAMP_INVALID;
        return code;
    }

    m_code = 0;
    return 0;
}

mfxI32 StartCodeIterator::FindStartCode(mfxU8 * (&pb), mfxU32 & size, mfxI32 & startCodeSize)
{
    mfxU32 zeroCount = 0;

    for (mfxU32 i = 0 ; i < (mfxU32)size; i++, pb++)
    {
        switch(pb[0])
        {
        case 0x00:
            zeroCount++;
            break;
        case 0x01:
            if (zeroCount >= 2)
            {
                startCodeSize = std::min(zeroCount + 1, 4u);
                size -= i + 1;
                pb++; // remove 0x01 symbol
                zeroCount = 0;
                if (size >= 1)
                {
                    return pb[0] & AVC_NAL_UNITTYPE_BITS_MASK;
                }
                else
                {
                    pb -= startCodeSize;
                    size += startCodeSize;
                    startCodeSize = 0;
                    return 0;
                }
            }
            zeroCount = 0;
            break;
        default:
            zeroCount = 0;
            break;
        }
    }

    zeroCount = std::min(zeroCount, 3u);
    pb -= zeroCount;
    size += zeroCount;
    zeroCount = 0;
    startCodeSize = 0;
    return 0;
}

void BytesSwapper::SwapMemory(mfxU8 *pDestination, mfxU32 &nDstSize, mfxU8 *pSource, mfxU32 nSrcSize)
{
    SwapMemoryAndRemovePreventingBytes(pDestination, nDstSize, pSource, nSrcSize);
}

NALUnitSplitter::NALUnitSplitter()
{
    memset(&m_bitstream, 0, sizeof(m_bitstream));
}

NALUnitSplitter::~NALUnitSplitter()
{
    Release();
}

void NALUnitSplitter::Init()
{
    Release();
}

void NALUnitSplitter::Reset()
{
    m_pStartCodeIter.Reset();
}

void NALUnitSplitter::Release()
{
}

mfxI32 NALUnitSplitter::CheckNalUnitType(mfxBitstream * source)
{
    return m_pStartCodeIter.CheckNalUnitType(source);
}

mfxI32 NALUnitSplitter::GetNalUnits(mfxBitstream * source, mfxBitstream * &destination)
{
    mfxI32 iCode = m_pStartCodeIter.GetNALUnit(source, &m_bitstream);

    if (iCode == 0)
    {
        destination = 0;
        return 0;
    }

    destination = &m_bitstream;
    return iCode;
}

/* temporal class definition */
class H264DwordPointer_
{
public:
    // Default constructor
    H264DwordPointer_(void)
    {
        m_pDest = NULL;
        m_nByteNum = 0;
        m_iCur = 0;
    }

    H264DwordPointer_ operator = (void *pDest)
    {
        m_pDest = (mfxU32 *) pDest;
        m_nByteNum = 0;
        m_iCur = 0;

        return *this;
    }

    // Increment operator
    H264DwordPointer_ &operator ++ (void)
    {
        if (4 == ++m_nByteNum)
        {
            *m_pDest = m_iCur;
            m_pDest += 1;
            m_nByteNum = 0;
            m_iCur = 0;
        }
        else
            m_iCur <<= 8;

        return *this;
    }

    mfxU8 operator = (mfxU8 nByte)
    {
        m_iCur = (m_iCur & ~0x0ff) | ((mfxU32) nByte);

        return nByte;
    }

protected:
    mfxU32 *m_pDest;                                            // pointer to destination buffer
    mfxU32 m_nByteNum;                                          // number of current byte in dword
    mfxU32 m_iCur;                                              // current dword
};

class H264SourcePointer_
{
public:
    // Default constructor
    H264SourcePointer_(void)
    {
        m_pSource = NULL;
        m_nRemovedBytes=0;
        m_nZeros=0;
    }

    H264SourcePointer_ &operator = (mfxU8 *pSource)
    {
        m_pSource = (mfxU8 *) pSource;

        m_nZeros = 0;
        m_nRemovedBytes = 0;

        return *this;
    }

    H264SourcePointer_ &operator ++ (void)
    {
        mfxU8 bCurByte = m_pSource[0];

        if (0 == bCurByte)
            m_nZeros += 1;
        else
        {
            if ((3 == bCurByte) && (2 <= m_nZeros))
                m_nRemovedBytes += 1;
            m_nZeros = 0;
        }

        m_pSource += 1;

        return *this;
    }

    bool IsPrevent(void)
    {
        if ((3 == m_pSource[0]) && (2 <= m_nZeros))
            return true;
        else
            return false;
    }

    operator mfxU8 (void)
    {
        return m_pSource[0];
    }

    mfxU32 GetRemovedBytes(void)
    {
        return m_nRemovedBytes;
    }

protected:
    mfxU8 *m_pSource;                                           // pointer to destination buffer
    mfxU32 m_nZeros;                                            // number of preceding zeros
    mfxU32 m_nRemovedBytes;                                     // number of removed bytes
};

void SwapMemoryAndRemovePreventingBytes(mfxU8 *pDestination, mfxU32 &nDstSize, mfxU8 *pSource, mfxU32 nSrcSize)
{
    H264DwordPointer_ pDst;
    H264SourcePointer_ pSrc;
    size_t i;

    // DwordPointer object is swapping written bytes
    // H264SourcePointer_ removes preventing start-code bytes

    // reset pointer(s)
    pSrc = pSource;
    pDst = pDestination;

    // first two bytes
    i = 0;
    while (i < std::min(2u, nSrcSize))
    {
        pDst = (mfxU8) pSrc;
        ++pDst;
        ++pSrc;
        ++i;
    }

    // do swapping
    while (i < (mfxU32) nSrcSize)
    {
        if (false == pSrc.IsPrevent())
        {
            pDst = (mfxU8) pSrc;
            ++pDst;
        }
        ++pSrc;
        ++i;
    }

    // write padding bytes
    nDstSize = nSrcSize - pSrc.GetRemovedBytes();
    while (nDstSize & 3)
    {
        pDst = (mfxU8) (0);
        ++nDstSize;
        ++pDst;
    }
}

} // namespace ProtectedLibrary
