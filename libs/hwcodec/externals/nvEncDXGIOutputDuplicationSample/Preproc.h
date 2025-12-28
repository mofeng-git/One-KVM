/*
 * Copyright (c) 2019, NVIDIA CORPORATION. All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *  * Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 *  * Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *  * Neither the name of NVIDIA CORPORATION nor the names of its
 *    contributors may be used to endorse or promote products derived
 *    from this software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS ``AS IS'' AND ANY
 * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
 * PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT OWNER OR
 * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
 * EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
 * PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
 * PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY
 * OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

#pragma once
#include <dxgi1_2.h>
#include <d3d11_2.h>
#include <unordered_map>
using namespace std;

class RGBToNV12
{
    /// Simple Preprocessor class
    /// Uses DXVAHD VideoProcessBlt to perform colorspace conversion
private:
    /// D3D11 device to be used for Processing
    ID3D11Device *m_pDev = nullptr;
    /// D3D11 device context to be used for Processing
    ID3D11DeviceContext* m_pCtx = nullptr;
    /// D3D11 video device to be used for Processing, obtained from d3d11 device
    ID3D11VideoDevice* m_pVid = nullptr;
    /// D3D11 video device context to be used for Processing, obtained from d3d11 device
    ID3D11VideoContext* m_pVidCtx = nullptr;
    /// DXVAHD video processor configured for processing.
    /// Needs to be reconfigured based on input and output textures for each Convert() call
    ID3D11VideoProcessor* m_pVP = nullptr;
    /// DXVAHD VpBlt output target. Obtained from the output texture passed to Convert()
    ID3D11VideoProcessorOutputView* m_pVPOut = nullptr;
    /// D3D11 video processor enumerator. Required to configure Video processor streams
    ID3D11VideoProcessorEnumerator* m_pVPEnum = nullptr;
    /// Mapping of Texture2D handle and corresponding Video Processor output view handle
    /// Optimization to avoid having to create video processor output views in each Convert() call
    std::unordered_map<ID3D11Texture2D*, ID3D11VideoProcessorOutputView*> viewMap;
    /// Input and Output Texture2D properties.
    /// Required to optimize Video Processor stream usage
    D3D11_TEXTURE2D_DESC m_inDesc = { 0 };
    D3D11_TEXTURE2D_DESC m_outDesc = { 0 };
private:
    /// Default Constructor
    RGBToNV12() 
    {
    }

public:
    /// Initialize Video Context
    HRESULT Init();
    /// Perform Colorspace conversion
    HRESULT Convert(ID3D11Texture2D* pRGB, ID3D11Texture2D*pYUV);
    /// Release all resources
    void Cleanup();

public:
    /// Constructor
    RGBToNV12(ID3D11Device *pDev, ID3D11DeviceContext *pCtx);
    /// Destructor. Release all resources before destroying object
    ~RGBToNV12() 
    {
        Cleanup(); 
    }

};


