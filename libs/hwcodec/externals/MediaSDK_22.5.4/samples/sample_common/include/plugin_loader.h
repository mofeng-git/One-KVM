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

#pragma once

#ifndef __PLUGIN_LOADER_H__
#define __PLUGIN_LOADER_H__

#include "vm/so_defs.h"
#include "sample_utils.h"
#include "plugin_utils.h"
//#include "mfx_plugin_module.h"
#include <iostream>
#include <iomanip> // for std::setfill, std::setw
#include <memory> // for std::unique_ptr

class MsdkSoModule
{
protected:
    msdk_so_handle m_module;
public:
    MsdkSoModule()
        : m_module(NULL)
    {
    }
    MsdkSoModule(const msdk_string & pluginName)
        : m_module(NULL)
    {
        m_module = msdk_so_load(pluginName.c_str());
        if (NULL == m_module)
        {
            MSDK_TRACE_ERROR(msdk_tstring(MSDK_CHAR("Failed to load shared module: ")) + pluginName);
        }
    }
    template <class T>
    T GetAddr(const std::string & fncName)
    {
        T pCreateFunc = reinterpret_cast<T>(msdk_so_get_addr(m_module, fncName.c_str()));
        if (NULL == pCreateFunc) {
            MSDK_TRACE_ERROR(msdk_tstring("Failed to get function addres: ") + fncName.c_str());
        }
        return pCreateFunc;
    }

    virtual ~MsdkSoModule()
    {
        if (m_module)
        {
            msdk_so_free(m_module);
            m_module = NULL;
        }
    }
};

/*
* Rationale: class to load+register any mediasdk plugin decoder/encoder/generic by given name
*/
class PluginLoader : public MFXPlugin
{
protected:
    mfxPluginType     ePluginType;

    mfxSession        m_session;
    mfxPluginUID      m_uid;

private:
    const msdk_char* msdkGetPluginName(const mfxPluginUID& guid)
    {
        if (AreGuidsEqual(guid, MFX_PLUGINID_HEVCD_SW))
            return MSDK_STRING("Intel (R) Media SDK plugin for HEVC DECODE");
        else if(AreGuidsEqual(guid, MFX_PLUGINID_HEVCD_HW))
            return MSDK_STRING("Intel (R) Media SDK HW plugin for HEVC DECODE");
        else if(AreGuidsEqual(guid, MFX_PLUGINID_HEVCE_SW))
            return MSDK_STRING("Intel (R) Media SDK plugin for HEVC ENCODE");
        else if(AreGuidsEqual(guid, MFX_PLUGINID_HEVCE_HW))
            return MSDK_STRING("Intel (R) Media SDK HW plugin for HEVC ENCODE");
        else if(AreGuidsEqual(guid, MFX_PLUGINID_VP8E_HW))
            return MSDK_STRING("Intel (R) Media SDK HW plugin for VP8 ENCODE");
        else if(AreGuidsEqual(guid, MFX_PLUGINID_VP8D_HW))
            return MSDK_STRING("Intel (R) Media SDK HW plugin for VP8 DECODE");
        else if(AreGuidsEqual(guid, MFX_PLUGINID_VP9E_HW))
            return MSDK_STRING("Intel (R) Media SDK HW plugin for VP9 ENCODE");
        else if(AreGuidsEqual(guid, MFX_PLUGINID_VP9D_HW))
            return MSDK_STRING("Intel (R) Media SDK HW plugin for VP9 DECODE");
        else if(AreGuidsEqual(guid, MFX_PLUGINID_H264LA_HW))
            return MSDK_STRING("Intel (R) Media SDK plugin for LA ENC");
        else if(AreGuidsEqual(guid, MFX_PLUGINID_ITELECINE_HW))
            return MSDK_STRING("Intel (R) Media SDK PTIR plugin (HW)");
        else if (AreGuidsEqual(guid, MFX_PLUGINID_HEVCE_GACC))
            return MSDK_STRING("Intel (R) Media SDK GPU-Accelerated plugin for HEVC ENCODE");
        else
#if (MFX_VERSION >= 1027) && !defined(_WIN32) && !defined(_WIN64)
        if (AreGuidsEqual(guid, MFX_PLUGINID_HEVC_FEI_ENCODE))
            return MSDK_STRING("Intel (R) Media SDK HW plugin for HEVC FEI ENCODE");
        else
#endif
            return MSDK_STRING("Unknown plugin");
    }

public:
    PluginLoader(mfxPluginType type, mfxSession session, const mfxPluginUID & uid, mfxU32 version, const mfxChar *pluginName, mfxU32 len)
        : ePluginType(type)
        , m_session()
        , m_uid()
    {
        mfxStatus sts = MFX_ERR_NONE;
        msdk_stringstream strStream;

        MSDK_MEMCPY(&m_uid, &uid, sizeof(mfxPluginUID));
        for (size_t i = 0; i != sizeof(mfxPluginUID); i++)
        {
            strStream << MSDK_STRING("0x") << std::setfill(MSDK_CHAR('0')) << std::setw(2) << std::hex << (int)m_uid.Data[i];
            if (i != (sizeof(mfxPluginUID)-1)) strStream << MSDK_STRING(", ");
        }

        if ((ePluginType == MFX_PLUGINTYPE_AUDIO_DECODE) ||
            (ePluginType == MFX_PLUGINTYPE_AUDIO_ENCODE))
        {
            // Audio plugins are not loaded by path
            sts = MFX_ERR_UNSUPPORTED;
        }
        else
        {
            sts = MFXVideoUSER_LoadByPath(session, &m_uid, version, pluginName, len);
        }

        if (MFX_ERR_NONE != sts)
        {
            MSDK_TRACE_ERROR(MSDK_STRING("Failed to load plugin from GUID, sts=") << sts << MSDK_STRING(": { ") << strStream.str().c_str() << MSDK_STRING(" } (") << msdkGetPluginName(m_uid) << MSDK_STRING(")"));
        }
        else
        {
            MSDK_TRACE_INFO(MSDK_STRING("Plugin was loaded from GUID"));
            m_session = session;
        }
    }

    PluginLoader(mfxPluginType type, mfxSession session, const mfxPluginUID & uid, mfxU32 version)
        : ePluginType(type)
        , m_session()
        , m_uid()
    {
        mfxStatus sts = MFX_ERR_NONE;
        msdk_stringstream strStream;

        MSDK_MEMCPY(&m_uid, &uid, sizeof(mfxPluginUID));
        for (size_t i = 0; i != sizeof(mfxPluginUID); i++)
        {
            strStream << MSDK_STRING("0x") << std::setfill(MSDK_CHAR('0')) << std::setw(2) << std::hex << (int)m_uid.Data[i];
            if (i != (sizeof(mfxPluginUID)-1)) strStream << MSDK_STRING(", ");
        }

        if ((ePluginType == MFX_PLUGINTYPE_AUDIO_DECODE) ||
            (ePluginType == MFX_PLUGINTYPE_AUDIO_ENCODE))
        {
            sts = MFXAudioUSER_Load(session, &m_uid, version);
        }
        else
        {
            sts = MFXVideoUSER_Load(session, &m_uid, version);
        }

        if (MFX_ERR_NONE != sts)
        {
            MSDK_TRACE_ERROR(MSDK_STRING("Failed to load plugin from GUID, sts=") << sts << MSDK_STRING(": { ") << strStream.str().c_str() << MSDK_STRING(" } (") << msdkGetPluginName(m_uid) << MSDK_STRING(")"));
        }
        else
        {
            MSDK_TRACE_INFO(MSDK_STRING("Plugin was loaded from GUID")<< MSDK_STRING(": { ") << strStream.str().c_str() << MSDK_STRING(" } (") << msdkGetPluginName(m_uid) << MSDK_STRING(")"));
            m_session = session;
        }
    }

    virtual ~PluginLoader()
    {
        mfxStatus sts = MFX_ERR_NONE;
        if (m_session)
        {
            if ((ePluginType == MFX_PLUGINTYPE_AUDIO_DECODE) ||
                (ePluginType == MFX_PLUGINTYPE_AUDIO_ENCODE))
            {
                sts = MFXAudioUSER_UnLoad(m_session, &m_uid);
            }
            else
            {
                sts = MFXVideoUSER_UnLoad(m_session, &m_uid);
            }

            if (sts != MFX_ERR_NONE)
            {
                  MSDK_TRACE_ERROR(MSDK_STRING("Failed to unload plugin from GUID, sts=") << sts);
            }
            else
            {
                MSDK_TRACE_INFO(MSDK_STRING("MFXBaseUSER_UnLoad(session=0x") << m_session << MSDK_STRING("), sts=") << sts);
            }
        }
    }

    bool IsOk() {
        return m_session != 0;
    }
    virtual mfxStatus PluginInit( mfxCoreInterface * /*core*/ ) {
        return MFX_ERR_NULL_PTR;
    }
    virtual mfxStatus PluginClose() {
        return MFX_ERR_NULL_PTR;
    }
    virtual mfxStatus GetPluginParam( mfxPluginParam * /*par*/ ) {
        return MFX_ERR_NULL_PTR;
    }
    virtual mfxStatus Execute( mfxThreadTask /*task*/, mfxU32 /*uid_p*/, mfxU32 /*uid_a*/ ) {
        return MFX_ERR_NULL_PTR;
    }
    virtual mfxStatus FreeResources( mfxThreadTask /*task*/, mfxStatus /*sts*/ ) {
        return MFX_ERR_NULL_PTR;
    }
    virtual void Release() {
    }
    virtual mfxStatus Close() {
        return MFX_ERR_NULL_PTR;
    }
    virtual mfxStatus SetAuxParams( void* /*auxParam*/, int /*auxParamSize*/ ) {
        return MFX_ERR_NULL_PTR;
    }
};

inline MFXPlugin * LoadPluginByType(mfxPluginType type, mfxSession session, const mfxPluginUID & uid, mfxU32 version, const mfxChar *pluginName, mfxU32 len) {
    std::unique_ptr<PluginLoader> plg(new PluginLoader (type, session, uid, version, pluginName, len));
    return plg->IsOk() ? plg.release() : NULL;
}

inline MFXPlugin * LoadPluginByGUID(mfxPluginType type, mfxSession session, const mfxPluginUID & uid, mfxU32 version) {
    std::unique_ptr<PluginLoader> plg(new PluginLoader (type, session, uid, version));
    return plg->IsOk() ? plg.release() : NULL;
}

inline MFXPlugin * LoadPlugin(mfxPluginType type, mfxSession session, const mfxPluginUID & uid, mfxU32 version, const mfxChar *pluginName, mfxU32 len) {
    return LoadPluginByType(type, session, uid, version, pluginName, len);
}

inline MFXPlugin * LoadPlugin(mfxPluginType type, mfxSession session, const mfxPluginUID & uid, mfxU32 version) {
    return LoadPluginByGUID(type, session, uid, version);
}
#endif // PLUGIN_LOADER