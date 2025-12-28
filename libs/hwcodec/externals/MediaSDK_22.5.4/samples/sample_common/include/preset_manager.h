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

#pragma once
enum EPresetModes
{
    PRESET_DEFAULT,
    PRESET_DSS,
    PRESET_CONF,
    PRESET_GAMING,
    PRESET_MAX_MODES
};

enum EPresetCodecs
{
    PRESET_AVC,
    PRESET_HEVC,
    PRESET_MAX_CODECS
};

struct CPresetParameters
{
    mfxU16 GopRefDist;

    mfxU16 TargetUsage;

    mfxU16 RateControlMethod;
    mfxU16 ExtBRCUsage;
    mfxU16 AsyncDepth;
    mfxU16 BRefType;
    mfxU16 AdaptiveMaxFrameSize;
    mfxU16 LowDelayBRC;

    mfxU16 IntRefType;
    mfxU16 IntRefCycleSize;
    mfxU16 IntRefQPDelta;
    mfxU16 IntRefCycleDist;

    mfxU16 WeightedPred;
    mfxU16 WeightedBiPred;

    bool EnableBPyramid;
    bool EnablePPyramid;
//    bool EnableLTR;
};

struct CDependentPresetParameters
{
    mfxU16 TargetKbps;
    mfxU16 MaxKbps;
    mfxU16 GopPicSize;
    mfxU16 BufferSizeInKB;
    mfxU16 LookAheadDepth;
    mfxU32 MaxFrameSize;
};

struct COutputPresetParameters : public CPresetParameters,CDependentPresetParameters
{

    msdk_string PresetName;

    void Clear()
    {
        memset(dynamic_cast<CPresetParameters*>(this), 0, sizeof(CPresetParameters));
        memset(dynamic_cast<CDependentPresetParameters*>(this), 0, sizeof(CDependentPresetParameters));
    }

    COutputPresetParameters()
    {
        Clear();
    }

    COutputPresetParameters(CPresetParameters src)
    {
        Clear();
        *(CPresetParameters*)this = src;
    }
};

class CPresetManager
{
public:
    ~CPresetManager();
    COutputPresetParameters GetPreset(EPresetModes mode, mfxU32 codecFourCC, mfxF64 fps, mfxU32 width, mfxU32 height, bool isHWLib);
    COutputPresetParameters GetBasicPreset(EPresetModes mode, mfxU32 codecFourCC);
    CDependentPresetParameters GetDependentPresetParameters(EPresetModes mode, mfxU32 codecFourCC, mfxF64 fps, mfxU32 width, mfxU32 height, mfxU16 targetUsage);

    static CPresetManager Inst;
    static EPresetModes PresetNameToMode(const msdk_char* name);
protected:
    CPresetManager();
    static CPresetParameters presets[PRESET_MAX_MODES][PRESET_MAX_CODECS];
    static msdk_string modesName[PRESET_MAX_MODES];
};

#define MODIFY_AND_PRINT_PARAM(paramName,presetName,shouldPrintPresetInfo) \
if (!paramName){ \
    paramName = presetParams.presetName; \
    if(shouldPrintPresetInfo){msdk_printf(MSDK_STRING(#presetName) MSDK_STRING(": %d\n"), (int)paramName);} \
}else{ \
    if(shouldPrintPresetInfo){msdk_printf(MSDK_STRING(#presetName) MSDK_STRING(": %d (original preset value: %d)\n"), (int)paramName, (int)presetParams.presetName);}}

#define MODIFY_AND_PRINT_PARAM_EXT(paramName,presetName,value,shouldPrintPresetInfo) \
if (!paramName){ \
    paramName = (value); \
    if(shouldPrintPresetInfo){msdk_printf(MSDK_STRING(#presetName) MSDK_STRING(": %d\n"), (int)paramName);} \
}else{ \
    if(shouldPrintPresetInfo){msdk_printf(MSDK_STRING(#presetName) MSDK_STRING(": %d (original preset value: %d)\n"), (int)paramName, (int)(value));}}

