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

#ifndef _ABSTRACT_SPL_H__
#define _ABSTRACT_SPL_H__

#include "mfxstructures.h"
#include "vm/strings_defs.h"

enum SliceTypeCode {
    TYPE_I = 0,
    TYPE_P = 1,
    TYPE_B = 2,
    TYPE_SKIP=3,
    TYPE_UNKNOWN=4
};

struct SliceSplitterInfo
{
    mfxU32   DataOffset;
    mfxU32   DataLength;
    mfxU32   HeaderLength;
    SliceTypeCode SliceType;
};

struct FrameSplitterInfo
{
    SliceSplitterInfo * Slice;   // array
    mfxU32     SliceNum;
    mfxU32     FirstFieldSliceNum;

    mfxU8  *   Data;    // including data of slices
    mfxU32     DataLength;
    mfxU64     TimeStamp;
};

class AbstractSplitter
{
public:

    AbstractSplitter()
    {}

    virtual ~AbstractSplitter()
    {}

    virtual mfxStatus Reset() = 0;

    virtual mfxStatus GetFrame(mfxBitstream * bs_in, FrameSplitterInfo ** frame) = 0;

    virtual mfxStatus PostProcessing(FrameSplitterInfo *frame, mfxU32 sliceNum) = 0;

    virtual void ResetCurrentState() = 0;
};

#endif // _ABSTRACT_SPL_H__
