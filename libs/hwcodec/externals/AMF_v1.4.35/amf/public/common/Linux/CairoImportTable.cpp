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
#include "CairoImportTable.h"
#include "public/common/TraceAdapter.h"
#include "../Thread.h"

using namespace amf;

#define GET_SO_ENTRYPOINT(m, h, f) m = reinterpret_cast<decltype(&f)>(amf_get_proc_address(h, #f)); \
    AMF_RETURN_IF_FALSE(nullptr != m, AMF_FAIL, L"Failed to acquire entrypoint %S", #f);

//-------------------------------------------------------------------------------------------------
CairoImportTable::CairoImportTable()
{}

//-------------------------------------------------------------------------------------------------
CairoImportTable::~CairoImportTable()
{
    UnloadFunctionsTable();
}

//-------------------------------------------------------------------------------------------------
AMF_RESULT CairoImportTable::LoadFunctionsTable()
{
    if (nullptr == m_hLibCairoSO)
    {
        m_hLibCairoSO = amf_load_library(L"libcairo.so.2");
        AMF_RETURN_IF_FALSE(nullptr != m_hLibCairoSO, AMF_FAIL, L"Failed to load libcairo.so.2");
    }

    GET_SO_ENTRYPOINT(m_cairo_image_surface_create_from_png_stream, m_hLibCairoSO, cairo_image_surface_create_from_png_stream);
    GET_SO_ENTRYPOINT(m_cairo_surface_destroy, m_hLibCairoSO, cairo_surface_destroy);
    GET_SO_ENTRYPOINT(m_cairo_image_surface_get_width, m_hLibCairoSO, cairo_image_surface_get_width);
    GET_SO_ENTRYPOINT(m_cairo_image_surface_get_height, m_hLibCairoSO, cairo_image_surface_get_height);
    GET_SO_ENTRYPOINT(m_cairo_image_surface_get_stride, m_hLibCairoSO, cairo_image_surface_get_stride);
    GET_SO_ENTRYPOINT(m_cairo_image_surface_get_format, m_hLibCairoSO, cairo_image_surface_get_format);
    GET_SO_ENTRYPOINT(m_cairo_image_surface_get_data, m_hLibCairoSO, cairo_image_surface_get_data);

    return AMF_OK;
}

void CairoImportTable::UnloadFunctionsTable()
{
    if (nullptr != m_hLibCairoSO)
    {
        amf_free_library(m_hLibCairoSO);
        m_hLibCairoSO = nullptr;
    }

    m_cairo_image_surface_create_from_png_stream = nullptr;
    m_cairo_surface_destroy = nullptr;
    m_cairo_image_surface_get_width = nullptr;
    m_cairo_image_surface_get_height = nullptr;
    m_cairo_image_surface_get_stride = nullptr;
    m_cairo_image_surface_get_format = nullptr;
    m_cairo_image_surface_get_data = nullptr;
}