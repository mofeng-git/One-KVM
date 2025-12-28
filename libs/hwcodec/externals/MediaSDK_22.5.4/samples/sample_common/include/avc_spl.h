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

#ifndef _AVC_SPL_H__
#define _AVC_SPL_H__

#include <vector>
#include <list>
#include <memory>

#include "abstract_splitter.h"

#include "avc_bitstream.h"
#include "avc_headers.h"
#include "avc_nal_spl.h"

namespace ProtectedLibrary
{

class AVCSlice : public SliceSplitterInfo
{
public:

    AVCSlice();

    AVCSliceHeader * GetSliceHeader();

    bool IsField() const {return m_sliceHeader.field_pic_flag != 0;}

    mfxI32 RetrievePicParamSetNumber(mfxU8 *pSource, mfxU32 nSourceSize);

    bool DecodeHeader(mfxU8 *pSource, mfxU32 nSourceSize);

    AVCHeadersBitstream *GetBitStream(void){return &m_bitStream;}

    AVCPicParamSet* m_picParamSet;
    AVCSeqParamSet* m_seqParamSet;
    AVCSeqParamSet* m_seqParamSetMvcEx;
    AVCSeqParamSetExtension* m_seqParamSetEx;

    mfxU64 m_dTime;

protected:
    AVCSliceHeader m_sliceHeader;
    AVCHeadersBitstream m_bitStream;

    void Reset();
};

class AVCFrameInfo
{
public:

    AVCFrameInfo();

    void Reset();

    AVCSlice  * m_slice;
    mfxU32      m_index;
};

class AVC_Spl : public AbstractSplitter
{
public:

    AVC_Spl();

    virtual ~AVC_Spl();

    virtual mfxStatus Reset();

    virtual mfxStatus GetFrame(mfxBitstream * bs_in, FrameSplitterInfo ** frame);

    virtual mfxStatus PostProcessing(FrameSplitterInfo *frame, mfxU32 sliceNum);

    void ResetCurrentState();

protected:
    std::unique_ptr<NALUnitSplitter> m_pNALSplitter;

    mfxStatus Init();

    void Close();

    mfxStatus ProcessNalUnit(mfxI32 nalType, mfxBitstream * destination);

    mfxStatus DecodeHeader(mfxBitstream * nalUnit);
    mfxStatus DecodeSEI(mfxBitstream * nalUnit);
    AVCSlice * DecodeSliceHeader(mfxBitstream * nalUnit);
    mfxStatus AddSlice(AVCSlice * pSlice);

    AVCFrameInfo * GetFreeFrame();

    mfxU8 * GetMemoryForSwapping(mfxU32 size);

    mfxStatus AddNalUnit(mfxBitstream * nalUnit);
    mfxStatus AddSliceNalUnit(mfxBitstream * nalUnit, AVCSlice * pSlice);
    bool IsFieldOfOneFrame(AVCFrameInfo * frame, const AVCSliceHeader * slice1, const AVCSliceHeader *slice2);

    bool                m_WaitForIDR;

    AVCHeaders     m_headers;
    std::unique_ptr<AVCFrameInfo> m_AUInfo;
    AVCFrameInfo * m_currentInfo;
    AVCSlice * m_pLastSlice;

    mfxBitstream * m_lastNalUnit;

    enum
    {
        BUFFER_SIZE = 1024 * 1024
    };

    std::vector<mfxU8>  m_currentFrame;
    std::vector<mfxU8>  m_swappingMemory;
    std::list<AVCSlice> m_slicesStorage;

    std::vector<SliceSplitterInfo>  m_slices;
    FrameSplitterInfo m_frame;
};

} // namespace ProtectedLibrary

#endif // _AVC_SPL_H__
