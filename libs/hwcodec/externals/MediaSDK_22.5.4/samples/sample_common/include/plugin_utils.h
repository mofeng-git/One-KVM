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

#ifndef __PLUGIN_UTILS_H__
#define __PLUGIN_UTILS_H__

#include "sample_defs.h"
#include "sample_types.h"

#if defined(_WIN32) || defined(_WIN64)
    #define MSDK_CPU_ROTATE_PLUGIN  MSDK_STRING("sample_rotate_plugin.dll")
    #define MSDK_OCL_ROTATE_PLUGIN  MSDK_STRING("sample_plugin_opencl.dll")
#else
    #define MSDK_CPU_ROTATE_PLUGIN  MSDK_STRING("libsample_rotate_plugin.so")
    #define MSDK_OCL_ROTATE_PLUGIN  MSDK_STRING("libsample_plugin_opencl.so")
#endif

typedef mfxI32 msdkComponentType;
enum
{
    MSDK_VDECODE = 0x0001,
    MSDK_VENCODE = 0x0002,
    MSDK_VPP     = 0x0004,
    MSDK_VENC    = 0x0008,
#if (MFX_VERSION >= 1027)
    MSDK_FEI     = 0x1000,
#endif
};

typedef enum {
    MFX_PLUGINLOAD_TYPE_GUID = 1,
    MFX_PLUGINLOAD_TYPE_FILE = 2
} MfxPluginLoadType;

struct sPluginParams
{
    mfxPluginUID      pluginGuid;
    mfxChar           strPluginPath[MSDK_MAX_FILENAME_LEN];
    MfxPluginLoadType type;
    sPluginParams()
    {
        MSDK_ZERO_MEMORY(*this);
    }
};

static const mfxPluginUID MSDK_PLUGINGUID_NULL = {{0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00}};

bool AreGuidsEqual(const mfxPluginUID& guid1, const mfxPluginUID& guid2);

const mfxPluginUID & msdkGetPluginUID(mfxIMPL impl, msdkComponentType type, mfxU32 uCodecid);

sPluginParams ParsePluginGuid(msdk_char* );
sPluginParams ParsePluginPath(msdk_char* );
mfxStatus ConvertStringToGuid(const msdk_string & strGuid, mfxPluginUID & mfxGuid);

#endif //__PLUGIN_UTILS_H__
