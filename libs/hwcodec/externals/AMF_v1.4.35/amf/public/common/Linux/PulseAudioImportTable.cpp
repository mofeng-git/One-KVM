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
///  @file   PulseAudioImprotTable.cpp
///  @brief  pulseaudio import table
///-------------------------------------------------------------------------
#include "PulseAudioImportTable.h"
#include "public/common/TraceAdapter.h"
#include "../Thread.h"

using namespace amf;

#define GET_SO_ENTRYPOINT(m, h, f) m = reinterpret_cast<decltype(&f)>(amf_get_proc_address(h, #f)); \
    AMF_RETURN_IF_FALSE(nullptr != m, AMF_FAIL, L"Failed to acquire entrypoint %S", #f);

//-------------------------------------------------------------------------------------------------
PulseAudioImportTable::PulseAudioImportTable()
{}

//-------------------------------------------------------------------------------------------------
PulseAudioImportTable::~PulseAudioImportTable()
{
    UnloadFunctionsTable();
}

//-------------------------------------------------------------------------------------------------
AMF_RESULT PulseAudioImportTable::LoadFunctionsTable()
{
    // Load pulseaudio simple api shared library and pulseaudio shared library.
    if (nullptr == m_hLibPulseSimpleSO)
    {
        m_hLibPulseSimpleSO = amf_load_library(L"libpulse-simple.so.0");
        AMF_RETURN_IF_FALSE(nullptr != m_hLibPulseSimpleSO, AMF_FAIL, L"Failed to load libpulse-simple.so.0");
    }

    if (nullptr == m_hLibPulseSO)
    {
        m_hLibPulseSO = amf_load_library(L"libpulse.so.0");
        AMF_RETURN_IF_FALSE(nullptr != m_hLibPulseSO, AMF_FAIL, L"Failed to load libpulse.so.0");
    }

    // Load pulseaudio mainloop functions.
    GET_SO_ENTRYPOINT(m_pPA_Mainloop_Free, m_hLibPulseSO, pa_mainloop_free);
    GET_SO_ENTRYPOINT(m_pPA_Mainloop_New, m_hLibPulseSO, pa_mainloop_new);
    GET_SO_ENTRYPOINT(m_pPA_Mainloop_Quit, m_hLibPulseSO, pa_mainloop_quit);
    GET_SO_ENTRYPOINT(m_pPA_Mainloop_Get_API, m_hLibPulseSO, pa_mainloop_get_api);
    GET_SO_ENTRYPOINT(m_pPA_Mainloop_Run, m_hLibPulseSO, pa_mainloop_run);

    // Load pulseaudio context functions.
    GET_SO_ENTRYPOINT(m_pPA_Context_Unref, m_hLibPulseSO, pa_context_unref);
    GET_SO_ENTRYPOINT(m_pPA_Context_Load_Module, m_hLibPulseSO, pa_context_load_module);
    GET_SO_ENTRYPOINT(m_pPA_Context_Unload_Module, m_hLibPulseSO, pa_context_unload_module);
    GET_SO_ENTRYPOINT(m_pPA_Context_New, m_hLibPulseSO, pa_context_new);
    GET_SO_ENTRYPOINT(m_pPA_Context_Get_State, m_hLibPulseSO, pa_context_get_state);
    GET_SO_ENTRYPOINT(m_pPA_Context_Set_State_Callback, m_hLibPulseSO, pa_context_set_state_callback);
    GET_SO_ENTRYPOINT(m_pPA_Context_Get_Server_Info, m_hLibPulseSO, pa_context_get_server_info);
    GET_SO_ENTRYPOINT(m_pPA_Context_Connect, m_hLibPulseSO, pa_context_connect);
    GET_SO_ENTRYPOINT(m_pPA_Context_Disconnect, m_hLibPulseSO, pa_context_disconnect);

    GET_SO_ENTRYPOINT(m_pPA_Context_Get_Sink_Info_By_Name, m_hLibPulseSO, pa_context_get_sink_info_by_name);
    GET_SO_ENTRYPOINT(m_pPA_Context_Get_Sink_Info_List, m_hLibPulseSO, pa_context_get_sink_info_list);
    GET_SO_ENTRYPOINT(m_pPA_Context_Get_Source_Info_List, m_hLibPulseSO, pa_context_get_source_info_list);

    // Load other pulse audio functions.
    GET_SO_ENTRYPOINT(m_pPA_Operation_Unref, m_hLibPulseSO, pa_operation_unref);
    GET_SO_ENTRYPOINT(m_pPA_Strerror, m_hLibPulseSO, pa_strerror);

    // Load pulse audio simple api functions.
    GET_SO_ENTRYPOINT(m_pPA_Simple_New, m_hLibPulseSimpleSO, pa_simple_new);
    GET_SO_ENTRYPOINT(m_pPA_Simple_Free, m_hLibPulseSimpleSO, pa_simple_free);
    GET_SO_ENTRYPOINT(m_pPA_Simple_Write, m_hLibPulseSimpleSO, pa_simple_write);
    GET_SO_ENTRYPOINT(m_pPA_Simple_Read, m_hLibPulseSimpleSO, pa_simple_read);
    GET_SO_ENTRYPOINT(m_pPA_Simple_Flush, m_hLibPulseSimpleSO, pa_simple_flush);
    GET_SO_ENTRYPOINT(m_pPA_Simple_Get_Latency, m_hLibPulseSimpleSO, pa_simple_get_latency);


    return AMF_OK;
}

void PulseAudioImportTable::UnloadFunctionsTable()
{
    if (nullptr != m_hLibPulseSimpleSO)
    {
        amf_free_library(m_hLibPulseSimpleSO);
        m_hLibPulseSO = nullptr;
    }

    if (nullptr != m_hLibPulseSO)
    {
        amf_free_library(m_hLibPulseSO);
        m_hLibPulseSO = nullptr;
    }

    m_pPA_Mainloop_Free = nullptr;
    m_pPA_Mainloop_Quit = nullptr;
    m_pPA_Mainloop_New = nullptr;
    m_pPA_Mainloop_Get_API = nullptr;
    m_pPA_Mainloop_Run = nullptr;

    // Context functions.
    m_pPA_Context_Unref = nullptr;
    m_pPA_Context_Load_Module = nullptr;
    m_pPA_Context_Unload_Module = nullptr;
    m_pPA_Context_New = nullptr;
    m_pPA_Context_Get_State = nullptr;
    m_pPA_Context_Set_State_Callback = nullptr;
    m_pPA_Context_Get_Server_Info = nullptr;
    m_pPA_Context_Connect = nullptr;
    m_pPA_Context_Disconnect = nullptr;

    m_pPA_Context_Get_Sink_Info_By_Name = nullptr;
    m_pPA_Context_Get_Sink_Info_List = nullptr;
    m_pPA_Context_Get_Source_Info_List = nullptr;

    // Others
    m_pPA_Operation_Unref = nullptr;
    m_pPA_Strerror = nullptr;

    // PulseAudio Simple API functions.
    m_pPA_Simple_New = nullptr;
    m_pPA_Simple_Free = nullptr;
    m_pPA_Simple_Write = nullptr;
    m_pPA_Simple_Read = nullptr;
    m_pPA_Simple_Flush = nullptr;
    m_pPA_Simple_Get_Latency = nullptr;
}