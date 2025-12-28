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

#ifndef __PARAMETERS_DUMPER_H__
#define __PARAMETERS_DUMPER_H__
#include "sample_defs.h"

class CParametersDumper
{
protected:
    static void SerializeFrameInfoStruct(msdk_ostream& sstr,msdk_string prefix,mfxFrameInfo& info);
    static void SerializeMfxInfoMFXStruct(msdk_ostream& sstr,msdk_string prefix,mfxInfoMFX& info);
    static void SerializeExtensionBuffer(msdk_ostream& sstr,msdk_string prefix,mfxExtBuffer* pExtBuffer);
    static void SerializeVPPCompInputStream(msdk_ostream& sstr, msdk_string prefix, mfxVPPCompInputStream& info);

    template <class T>
    static mfxStatus GetUnitParams(T* pMfxUnit, const mfxVideoParam* pPresetParams, mfxVideoParam* pOutParams)
    {
        memset(pOutParams,0, sizeof(mfxVideoParam));
        mfxExtBuffer** paramsArray = new mfxExtBuffer*[pPresetParams->NumExtParam];
        for (int paramNum = 0; paramNum < pPresetParams->NumExtParam; paramNum++)
        {
            mfxExtBuffer* buf = pPresetParams->ExtParam[paramNum];
            mfxExtBuffer* newBuf = (mfxExtBuffer*)new mfxU8[buf->BufferSz];
            memset(newBuf, 0, buf->BufferSz);
            newBuf->BufferId = buf->BufferId;
            newBuf->BufferSz = buf->BufferSz;
            paramsArray[paramNum]=newBuf;
        }
        pOutParams->NumExtParam = pPresetParams->NumExtParam;
        pOutParams->ExtParam = paramsArray;

        mfxStatus sts = pMfxUnit->GetVideoParam(pOutParams);
        MSDK_CHECK_STATUS_SAFE(sts, "Cannot read configuration from encoder: GetVideoParam failed", ClearExtBuffs(pOutParams));

        return MFX_ERR_NONE;
    }

    static void ClearExtBuffs(mfxVideoParam* params)
    {
        // Cleaning params array
        for (int paramNum = 0; paramNum < params->NumExtParam; paramNum++)
        {
            delete[] params->ExtParam[paramNum];
        }
        delete[] params->ExtParam;
        params->ExtParam = NULL;
        params->NumExtParam = 0;
    }

public:
    static void SerializeVideoParamStruct(msdk_ostream& sstr,msdk_string sectionName,mfxVideoParam& info,bool shouldUseVPPSection=false);
    static mfxStatus DumpLibraryConfiguration(msdk_string fileName, MFXVideoDECODE* pMfxDec, MFXVideoVPP* pMfxVPP, MFXVideoENCODE* pMfxEnc,
        const mfxVideoParam* pDecoderPresetParams, const mfxVideoParam* pVPPPresetParams, const mfxVideoParam* pEncoderPresetParams);
    static void ShowConfigurationDiff(msdk_ostream& sstr1, msdk_ostream& sstr2);
};
#endif
