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

#include "preset_manager.h"
#include "mfxvideo.h"
#include "brc_routines.h"

CPresetManager CPresetManager::Inst;

msdk_string CPresetManager::modesName[PRESET_MAX_MODES] =
{
    MSDK_STRING("Default"),
    MSDK_STRING("DSS"),
    MSDK_STRING("Conference"),
    MSDK_STRING("Gaming"),
};

//GopRefDist, TargetUsage, RateControlMethod, ExtBRCType, AsyncDepth, BRefType
// AdaptiveMaxFrameSize, LowDelayBRC, IntRefType, IntRefCycleSize, IntRefQPDelta, IntRefCycleDist, WeightedPred, WeightedBiPred, EnableBPyramid, EnablePPyramid

CPresetParameters CPresetManager::presets[PRESET_MAX_MODES][PRESET_MAX_CODECS] =
{
    // Default
    {
        {4, MFX_TARGETUSAGE_BALANCED, MFX_RATECONTROL_VBR, EXTBRC_DEFAULT, 4, MFX_B_REF_PYRAMID,
         0, 0, 0, 0, 0, 0, MFX_WEIGHTED_PRED_UNKNOWN, MFX_WEIGHTED_PRED_UNKNOWN, 0, 0},
        {0, MFX_TARGETUSAGE_BALANCED, MFX_RATECONTROL_VBR, EXTBRC_DEFAULT, 4, 0,
         0, 0, 0, 0, 0, 0, MFX_WEIGHTED_PRED_UNKNOWN, MFX_WEIGHTED_PRED_UNKNOWN, 0, 0 }
    },
    // DSS
    {
        {1, MFX_TARGETUSAGE_BALANCED, MFX_RATECONTROL_QVBR, EXTBRC_DEFAULT, 1, 0,
         0, 0, 0, 0, 0, 0, MFX_WEIGHTED_PRED_UNKNOWN, MFX_WEIGHTED_PRED_UNKNOWN, 0, 0 },
        {1, MFX_TARGETUSAGE_BALANCED, MFX_RATECONTROL_QVBR, EXTBRC_DEFAULT, 1, 0,
         0, 0, 0, 0, 0, 0, MFX_WEIGHTED_PRED_UNKNOWN, MFX_WEIGHTED_PRED_UNKNOWN, 1, 1 },
    },
    // Conference
    {
        {1, MFX_TARGETUSAGE_BALANCED, MFX_RATECONTROL_VCM, EXTBRC_DEFAULT, 1, 0,
        0, 0, 0, 0, 0, 0, MFX_WEIGHTED_PRED_UNKNOWN, MFX_WEIGHTED_PRED_UNKNOWN, 0, 0 },
        {1, MFX_TARGETUSAGE_BALANCED, MFX_RATECONTROL_VBR, EXTBRC_ON, 1, 0,
        0, 0, 0, 0, 0, 0, MFX_WEIGHTED_PRED_UNKNOWN, MFX_WEIGHTED_PRED_UNKNOWN, 0, 0 },
    },
    // Gaming
    {
        {1, MFX_TARGETUSAGE_BALANCED, MFX_RATECONTROL_QVBR, EXTBRC_DEFAULT, 1, 0,
        MFX_CODINGOPTION_ON, MFX_CODINGOPTION_ON, MFX_REFRESH_HORIZONTAL, 8, 0, 4, MFX_WEIGHTED_PRED_UNKNOWN, MFX_WEIGHTED_PRED_UNKNOWN, 0, 0 },
        {1, MFX_TARGETUSAGE_BALANCED, MFX_RATECONTROL_VBR, EXTBRC_ON, 1, 0,
        MFX_CODINGOPTION_ON, MFX_CODINGOPTION_ON, MFX_REFRESH_HORIZONTAL, 8, 0, 4, MFX_WEIGHTED_PRED_UNKNOWN, MFX_WEIGHTED_PRED_UNKNOWN, 0, 0 },
    }
};

CPresetManager::CPresetManager()
{
}


CPresetManager::~CPresetManager()
{
}

COutputPresetParameters CPresetManager::GetPreset(EPresetModes mode, mfxU32 codecFourCC, mfxF64 fps, mfxU32 width, mfxU32 height, bool isHWLib)
{
    COutputPresetParameters retVal = GetBasicPreset(mode, codecFourCC);
    *(dynamic_cast<CDependentPresetParameters*>(&retVal)) = GetDependentPresetParameters(mode, codecFourCC, fps, width, height,retVal.TargetUsage);

    if (!isHWLib)
    {
        // These features are unsupported in SW library
        retVal.WeightedBiPred = 0;
        retVal.WeightedPred = 0;
    }

    return retVal;
}

COutputPresetParameters CPresetManager::GetBasicPreset(EPresetModes mode, mfxU32 codecFourCC)
{
    COutputPresetParameters retVal;

    if (mode < 0 || mode >= PRESET_MAX_MODES)
    {
        mode = PRESET_DEFAULT;
    }

    // Reading basic preset values
    switch (codecFourCC)
    {
    case MFX_CODEC_AVC:
        retVal = presets[mode][PRESET_AVC];
        break;
    case MFX_CODEC_HEVC:
        retVal = presets[mode][PRESET_HEVC];
        break;
    default:
        if (mode != PRESET_DEFAULT)
        {
            msdk_printf(MSDK_STRING("WARNING: Presets are available for h.264 or h.265 codecs only. Request for particular preset is ignored.\n"));
        }

        if (codecFourCC != MFX_CODEC_JPEG)
        {
            retVal.TargetUsage = MFX_TARGETUSAGE_BALANCED;
            retVal.RateControlMethod = MFX_RATECONTROL_CBR;
        }
        retVal.AsyncDepth = 4;
        return retVal;
    }

    retVal.PresetName = modesName[mode];
    return retVal;
}

CDependentPresetParameters CPresetManager::GetDependentPresetParameters(EPresetModes mode, mfxU32 codecFourCC, mfxF64 fps, mfxU32 width, mfxU32 height,mfxU16 targetUsage)
{
    CDependentPresetParameters retVal = {};
    retVal.TargetKbps = codecFourCC != MFX_CODEC_JPEG ? CalculateDefaultBitrate(codecFourCC, targetUsage, width, height, fps) : 0;

    if (codecFourCC == MFX_CODEC_AVC || codecFourCC == MFX_CODEC_HEVC)
    {
        // Calculating dependent preset values
        retVal.MaxKbps = (mode == PRESET_GAMING ? (mfxU16)(1.2*retVal.TargetKbps) : 0);
        retVal.GopPicSize = (mode == PRESET_GAMING || mode == PRESET_DEFAULT ? 0 : (mfxU16)(2 * fps));
        retVal.BufferSizeInKB = (mode == PRESET_DEFAULT ? 0 : retVal.TargetKbps); // 1 second buffers
        retVal.LookAheadDepth = 0; // Enable this setting if LA BRC will be enabled
        retVal.MaxFrameSize = (mode == PRESET_GAMING ? (mfxU32)(retVal.TargetKbps*0.166) : 0);
    }
    return retVal;
}

EPresetModes CPresetManager::PresetNameToMode(const msdk_char* name)
{
    for (int i = 0; i < PRESET_MAX_MODES; i++)
    {
        if (!msdk_stricmp(modesName[i].c_str(), name))
        {
            return (EPresetModes)i;
        }
    }
    return PRESET_MAX_MODES;
}