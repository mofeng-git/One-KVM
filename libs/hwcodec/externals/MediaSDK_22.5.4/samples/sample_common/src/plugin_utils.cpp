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

#include "mfx_samples_config.h"

#include "plugin_utils.h"
#include "mfxvp8.h"
#include <sstream>
#include <map>

bool AreGuidsEqual(const mfxPluginUID& guid1, const mfxPluginUID& guid2)
{
    for(size_t i = 0; i != sizeof(mfxPluginUID); i++)
    {
        if (guid1.Data[i] != guid2.Data[i])
            return false;
    }
    return true;
}

mfxStatus ConvertStringToGuid(const msdk_string & strGuid, mfxPluginUID & mfxGuid)
{
    mfxStatus sts = MFX_ERR_NONE;

    // Check if symbolic GUID value
    std::map<msdk_string, mfxPluginUID> uid;
    uid[MSDK_STRING("hevcd_sw")] = MFX_PLUGINID_HEVCD_SW;
    uid[MSDK_STRING("hevcd_hw")] = MFX_PLUGINID_HEVCD_HW;

    uid[MSDK_STRING("hevce_sw")] = MFX_PLUGINID_HEVCE_SW;
    uid[MSDK_STRING("hevce_gacc")] = MFX_PLUGINID_HEVCE_GACC;
    uid[MSDK_STRING("hevce_hw")] = MFX_PLUGINID_HEVCE_HW;

    uid[MSDK_STRING("vp8d_hw")] = MFX_PLUGINID_VP8D_HW;
    uid[MSDK_STRING("vp8e_hw")] = MFX_PLUGINID_VP8E_HW;

    uid[MSDK_STRING("vp9d_hw")] = MFX_PLUGINID_VP9D_HW;
    uid[MSDK_STRING("vp9e_hw")] = MFX_PLUGINID_VP9E_HW;

    uid[MSDK_STRING("camera_hw")] = MFX_PLUGINID_CAMERA_HW;

    uid[MSDK_STRING("capture_hw")] = MFX_PLUGINID_CAPTURE_HW;

    uid[MSDK_STRING("ptir_hw")] = MFX_PLUGINID_ITELECINE_HW;
    uid[MSDK_STRING("h264_la_hw")] = MFX_PLUGINID_H264LA_HW;
    uid[MSDK_STRING("aacd")] = MFX_PLUGINID_AACD;
    uid[MSDK_STRING("aace")] = MFX_PLUGINID_AACE;

    uid[MSDK_STRING("hevce_fei_hw")] = MFX_PLUGINID_HEVCE_FEI_HW;

    if (uid.find(strGuid) == uid.end())
    {
        mfxGuid = MSDK_PLUGINGUID_NULL;
        sts = MFX_ERR_UNKNOWN;
    }
    else
    {
        mfxGuid = uid[strGuid];
        sts = MFX_ERR_NONE;
    }

    // Check if plain GUID value
    if (sts)
    {
        if (strGuid.size() != 32)
        {
            sts = MFX_ERR_UNKNOWN;
        }
        else
        {
            for (size_t i = 0; i < 16; i++)
            {
                unsigned int xx = 0;
                msdk_stringstream ss;
                ss << std::hex << strGuid.substr(i * 2, 2);
                ss >> xx;
                mfxGuid.Data[i] = (mfxU8)xx;
            }
            sts = MFX_ERR_NONE;
        }
    }
    return sts;
}

const mfxPluginUID & msdkGetPluginUID(mfxIMPL impl, msdkComponentType type, mfxU32 uCodecid)
{
    if (impl == MFX_IMPL_SOFTWARE)
    {
        switch(type)
        {
        case MSDK_VDECODE:
            switch(uCodecid)
            {
            case MFX_CODEC_HEVC:
                return MFX_PLUGINID_HEVCD_SW;
            }
            break;
        case MSDK_VENCODE:
            switch(uCodecid)
            {
            case MFX_CODEC_HEVC:
                return MFX_PLUGINID_HEVCE_SW;
            }
            break;
        }
    }
    else
    {
        switch(type)
        {
        case MSDK_VENCODE:
            switch(uCodecid)
            {
            case MFX_CODEC_VP8:
                return MFX_PLUGINID_VP8E_HW;
            }
            break;
#if MFX_VERSION >= 1027
        case (MSDK_VENCODE | MSDK_FEI):
            switch (uCodecid)
            {
            case MFX_CODEC_HEVC:
                return MFX_PLUGINID_HEVC_FEI_ENCODE;
            }
            break;
#endif
        case MSDK_VENC:
            switch(uCodecid)
            {
            case MFX_CODEC_HEVC:
                return MFX_PLUGINID_HEVCE_FEI_HW;   // HEVC FEI uses ENC interface
            }
            break;
        }
    }

    return MSDK_PLUGINGUID_NULL;
}

sPluginParams ParsePluginGuid(msdk_char* strPluginGuid)
{
    sPluginParams pluginParams;
    mfxPluginUID uid;
    mfxStatus sts = ConvertStringToGuid(strPluginGuid, uid);

    if (sts == MFX_ERR_NONE)
    {
        pluginParams.type = MFX_PLUGINLOAD_TYPE_GUID;
        pluginParams.pluginGuid = uid;
    }

    return pluginParams;
}

sPluginParams ParsePluginPath(msdk_char* strPluginGuid)
{
    sPluginParams pluginParams;

    msdk_char tmpVal[MSDK_MAX_FILENAME_LEN];
    msdk_opt_read(strPluginGuid, tmpVal);

    MSDK_MAKE_BYTE_STRING(tmpVal, pluginParams.strPluginPath);
    pluginParams.type = MFX_PLUGINLOAD_TYPE_FILE;

    return pluginParams;
}
