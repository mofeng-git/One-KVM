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
#ifndef __AVC_BITSTREAM_H_
#define __AVC_BITSTREAM_H_

#include "mfxstructures.h"
#include "avc_structures.h"
#include "avc_headers.h"

namespace ProtectedLibrary
{

#define AVCPeek1Bit(current_data, offset) \
    ((current_data[0] >> (offset)) & 1)

#define AVCDrop1Bit(current_data, offset) \
{ \
    offset -= 1; \
    if (offset < 0) \
    { \
        offset = 31; \
        current_data += 1; \
    } \
}

// NAL unit definitions
enum
{
    NAL_STORAGE_IDC_BITS   = 0x60,
    NAL_UNITTYPE_BITS      = 0x1f
};

class AVCBaseBitstream
{
public:

    AVCBaseBitstream();
    AVCBaseBitstream(mfxU8 * const pb, const mfxU32 maxsize);
    virtual ~AVCBaseBitstream();

    // Reset the bitstream with new data pointer
    void Reset(mfxU8 * const pb, mfxU32 maxsize);
    void Reset(mfxU8 * const pb, mfxI32 offset, mfxU32 maxsize);

    inline mfxU32 GetBits(mfxU32 nbits);

    // Read one VLC mfxI32 or mfxU32 value from bitstream
    mfxI32 GetVLCElement(bool bIsSigned);

    // Reads one bit from the buffer.
    inline mfxU32 Get1Bit();

    // Check amount of data
    bool More_RBSP_Data();

    inline mfxU32 BytesDecoded();

    inline mfxU32 BitsDecoded();

    inline mfxU32 BytesLeft();

    mfxStatus GetNALUnitType(NAL_Unit_Type &uNALUnitType, mfxU8 &uNALStorageIDC);
    void AlignPointerRight(void);

protected:
    mfxU32 *m_pbs;                                              // pointer to the current position of the buffer.
    mfxI32 m_bitOffset;                                         // the bit position (0 to 31) in the dword pointed by m_pbs.
    mfxU32 *m_pbsBase;                                          // pointer to the first byte of the buffer.
    mfxU32 m_maxBsSize;                                         // maximum buffer size in bytes.
};

class AVCHeadersBitstream : public AVCBaseBitstream
{
public:

    AVCHeadersBitstream();
    AVCHeadersBitstream(mfxU8 * const pb, const mfxU32 maxsize);


    // Decode sequence parameter set
    mfxStatus GetSequenceParamSet(AVCSeqParamSet *sps);
    // Decode sequence parameter set extension
    mfxStatus GetSequenceParamSetExtension(AVCSeqParamSetExtension *sps_ex);

    // Decoding picture's parameter set functions
    mfxStatus GetPictureParamSetPart1(AVCPicParamSet *pps);
    mfxStatus GetPictureParamSetPart2(AVCPicParamSet *pps, const AVCSeqParamSet *sps);

    mfxStatus GetSliceHeaderPart1(AVCSliceHeader *pSliceHeader);
    // Decoding slice header functions
    mfxStatus GetSliceHeaderPart2(AVCSliceHeader *hdr, // slice header read goes here
                               const AVCPicParamSet *pps,
                               const AVCSeqParamSet *sps); // from slice header NAL unit

    mfxStatus GetSliceHeaderPart3(AVCSliceHeader *hdr, // slice header read goes here
                               PredWeightTable *pPredWeight_L0, // L0 weight table goes here
                               PredWeightTable *pPredWeight_L1, // L1 weight table goes here
                               RefPicListReorderInfo *pReorderInfo_L0,
                               RefPicListReorderInfo *pReorderInfo_L1,
                               AdaptiveMarkingInfo *pAdaptiveMarkingInfo,
                               const AVCPicParamSet *pps,
                               const AVCSeqParamSet *sps,
                               mfxU8 NALRef_idc); // from slice header NAL unit


    mfxStatus GetNalUnitPrefix(AVCNalExtension *pExt, mfxU32 NALRef_idc);

    mfxI32 GetSEI(const HeaderSet<AVCSeqParamSet> & sps, mfxI32 current_sps, AVCSEIPayLoad *spl);

private:

    mfxStatus GetNalUnitExtension(AVCNalExtension *pExt);

    void GetScalingList4x4(AVCScalingList4x4 *scl, mfxU8 *def, mfxU8 *scl_type);
    void GetScalingList8x8(AVCScalingList8x8 *scl, mfxU8 *def, mfxU8 *scl_type);

    mfxI32 GetSEIPayload(const HeaderSet<AVCSeqParamSet> & sps, mfxI32 current_sps, AVCSEIPayLoad *spl);
    mfxI32 recovery_point(const HeaderSet<AVCSeqParamSet> & sps, mfxI32 current_sps, AVCSEIPayLoad *spl);
    mfxI32 reserved_sei_message(const HeaderSet<AVCSeqParamSet> & sps, mfxI32 current_sps, AVCSEIPayLoad *spl);

    mfxStatus GetVUIParam(AVCSeqParamSet *sps);
    mfxStatus GetHRDParam(AVCSeqParamSet *sps);
};


void SetDefaultScalingLists(AVCSeqParamSet * sps);

extern const mfxU32 bits_data[];


#define _avcGetBits(current_data, offset, nbits, data) \
{ \
    mfxU32 x; \
 \
    SAMPLE_ASSERT((nbits) > 0 && (nbits) <= 32); \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
 \
    offset -= (nbits); \
 \
    if (offset >= 0) \
    { \
        x = current_data[0] >> (offset + 1); \
    } \
    else \
    { \
        offset += 32; \
 \
        x = current_data[1] >> (offset); \
        x >>= 1; \
        x += current_data[0] << (31 - offset); \
        current_data++; \
    } \
 \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
 \
    (data) = x & bits_data[nbits]; \
}

#define avcGetBits1(current_data, offset, data) \
{ \
    data = ((current_data[0] >> (offset)) & 1);  \
    offset -= 1; \
    if (offset < 0) \
    { \
        offset = 31; \
        current_data += 1; \
    } \
}

#define avcUngetNBits(current_data, offset, nbits) \
{ \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
 \
    offset += (nbits); \
    if (offset > 31) \
    { \
        offset -= 32; \
        current_data--; \
    } \
 \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
}

#define avcGetNBits( current_data, offset, nbits, data) \
    _avcGetBits(current_data, offset, nbits, data);

inline mfxU32 AVCBaseBitstream::GetBits(mfxU32 nbits)
{
    mfxU32 w, n = nbits;

    avcGetNBits(m_pbs, m_bitOffset, n, w);
    return w;
}

inline mfxU32 AVCBaseBitstream::Get1Bit()
{
    mfxU32 w;
    avcGetBits1(m_pbs, m_bitOffset, w);
    return w;

} // AVCBitstream::Get1Bit()

inline mfxU32 AVCBaseBitstream::BytesDecoded()
{
    return static_cast<mfxU32>((mfxU8*)m_pbs - (mfxU8*)m_pbsBase) +
            ((31 - m_bitOffset) >> 3);
}

inline mfxU32 AVCBaseBitstream::BytesLeft()
{
    return ((mfxI32)m_maxBsSize - (mfxI32) BytesDecoded());
}

} // namespace ProtectedLibrary

#endif // __AVC_BITSTREAM_H_
