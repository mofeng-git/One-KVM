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

#ifndef __AVC_HEADERS_H
#define __AVC_HEADERS_H

#include "avc_structures.h"
#include <vector>

namespace ProtectedLibrary
{

template <typename T>
class HeaderSet
{
public:

    HeaderSet()
        : m_currentID(-1)
    {
    }

    ~HeaderSet()
    {
        Reset();
    }

    void AddHeader(T* hdr)
    {
        mfxU32 id = hdr->GetID();
        if (id >= m_header.size())
        {
            m_header.resize(id + 1);
        }

        delete m_header[id];

        m_header[id] = new T();
        *(m_header[id]) = *hdr;
    }

    T * GetHeader(mfxU32 id)
    {
        if (id >= m_header.size())
            return 0;

        return m_header[id];
    }

    void RemoveHeader(mfxU32 id)
    {
        if (id >= m_header.size())
            return;

        delete m_header[id];
        m_header[id] = 0;
    }

    void RemoveHeader(T * hdr)
    {
        if (!hdr)
            return;

        RemoveHeader(hdr->GetID());
    }

    const T * GetHeader(mfxU32 id) const
    {
        if (id >= m_header.size())
            return 0;

        return m_header[id];
    }

    void Reset()
    {
        for (mfxU32 i = 0; i < m_header.size(); i++)
        {
            delete m_header[i];
            m_header[i]=0;
        }
    }

    void SetCurrentID(mfxU32 id)
    {
        m_currentID = id;
    }

    mfxI32 GetCurrrentID() const
    {
        return m_currentID;
    }

    T * GetCurrentHeader()
    {
        if (m_currentID == -1)
            return 0;

        return GetHeader(m_currentID);
    }

    const T * GetCurrentHeader() const
    {
        if (m_currentID == -1)
            return 0;

        return GetHeader(m_currentID);
    }

private:
    std::vector<T*>           m_header;
    mfxI32                    m_currentID;
};

/****************************************************************************************************/
// Headers stuff
/****************************************************************************************************/
class AVCHeaders
{
public:

    void Reset()
    {
        m_SeqParams.Reset();
        m_SeqExParams.Reset();
        m_SeqParamsMvcExt.Reset();
        m_PicParams.Reset();
        m_SEIParams.Reset();
    }

    HeaderSet<AVCSeqParamSet>             m_SeqParams;
    HeaderSet<AVCSeqParamSetExtension>    m_SeqExParams;
    HeaderSet<AVCSeqParamSet>             m_SeqParamsMvcExt;
    HeaderSet<AVCPicParamSet>             m_PicParams;
    HeaderSet<AVCSEIPayLoad>              m_SEIParams;
    AVCNalExtension                       m_nalExtension;
};

} //namespace ProtectedLibrary

#endif // __AVC_HEADERS_H
