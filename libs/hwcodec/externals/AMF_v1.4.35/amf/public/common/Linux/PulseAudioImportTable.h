//
// Notice Regarding Standards.  AMD does not provide a license or sublicense to
// any Intellectual Property Rights relating to any standards, including but not
// limited to any audio and/or video codec technologies such as MPEG-2, MPEG-4;
// AVC/H.264; HEVC/H.265; AAC decode/FFMPEG; AAC encode/FFMPEG; VC-1; and MP3
// (collectively, the "Media Technologies"). For clarity, you will pay any
// royalties due for such third party technologies, which may include the Media
// Technologies that are owed as a result of AMD providing the Software to you.
//
// MIT license
//
// Copyright (c) 2018 Advanced Micro Devices, Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//
///-------------------------------------------------------------------------
///  @file   PulseAudioImportTable.h
///  @brief  pulseaudio import table
///-------------------------------------------------------------------------
#pragma once

#include "../../include/core/Result.h"

#include <memory>
#include <pulse/simple.h>
#include <pulse/pulseaudio.h>

struct PulseAudioImportTable{

    PulseAudioImportTable();
    ~PulseAudioImportTable();

    AMF_RESULT LoadFunctionsTable();
    void UnloadFunctionsTable();

    // PulseAudio functions.
    // Mainloop functions.
    decltype(&pa_mainloop_free)              m_pPA_Mainloop_Free = nullptr;
    decltype(&pa_mainloop_quit)              m_pPA_Mainloop_Quit = nullptr;
    decltype(&pa_mainloop_new)               m_pPA_Mainloop_New = nullptr;
    decltype(&pa_mainloop_get_api)           m_pPA_Mainloop_Get_API = nullptr;
    decltype(&pa_mainloop_run)               m_pPA_Mainloop_Run = nullptr;

    // Context functions.
    decltype(&pa_context_unref)              m_pPA_Context_Unref = nullptr;
    decltype(&pa_context_load_module)        m_pPA_Context_Load_Module = nullptr;
    decltype(&pa_context_unload_module)      m_pPA_Context_Unload_Module = nullptr;
    decltype(&pa_context_new)                m_pPA_Context_New = nullptr;
    decltype(&pa_context_get_state)          m_pPA_Context_Get_State = nullptr;
    decltype(&pa_context_set_state_callback) m_pPA_Context_Set_State_Callback = nullptr;
    decltype(&pa_context_get_server_info)    m_pPA_Context_Get_Server_Info = nullptr;
    decltype(&pa_context_connect)            m_pPA_Context_Connect = nullptr;
    decltype(&pa_context_disconnect)         m_pPA_Context_Disconnect = nullptr;

    decltype(&pa_context_get_sink_info_by_name) m_pPA_Context_Get_Sink_Info_By_Name = nullptr;
    decltype(&pa_context_get_sink_info_list)    m_pPA_Context_Get_Sink_Info_List = nullptr;
    decltype(&pa_context_get_source_info_list)  m_pPA_Context_Get_Source_Info_List = nullptr;

    // Others
    decltype(&pa_operation_unref)            m_pPA_Operation_Unref = nullptr;
    decltype(&pa_strerror)                   m_pPA_Strerror = nullptr;

    // PulseAudio Simple API functions.
    decltype(&pa_simple_new)                 m_pPA_Simple_New = nullptr;
    decltype(&pa_simple_free)                m_pPA_Simple_Free = nullptr;
    decltype(&pa_simple_write)               m_pPA_Simple_Write = nullptr;
    decltype(&pa_simple_read)                m_pPA_Simple_Read = nullptr;
    decltype(&pa_simple_flush)               m_pPA_Simple_Flush = nullptr;
    decltype(&pa_simple_get_latency)         m_pPA_Simple_Get_Latency = nullptr;

    amf_handle                               m_hLibPulseSO = nullptr;
    amf_handle                               m_hLibPulseSimpleSO = nullptr;
};

typedef std::shared_ptr<PulseAudioImportTable> PulseAudioImportTablePtr;