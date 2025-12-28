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
// Copyright (c) 2020 Advanced Micro Devices, Inc. All rights reserved.
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

#include "VirtualMicrophoneAudioInput.h"
#include "public/common/TraceAdapter.h"
#if defined(_WIN32)
//-------------------------------------------------------------------------------------------------
#define AMF_FACILITY L"VirtualMicrophoneAudioInput"

//-------------------------------------------------------------------------------------------------
VirtualMicrophoneAudioInput::VirtualMicrophoneAudioInput(): 
	m_pAudioManager (nullptr), 
	m_pVirtualAudioInput (nullptr)
#ifdef WIN32
    , m_bCoInitializeSucceeded(false)
#endif
{
}
//-------------------------------------------------------------------------------------------------
VirtualMicrophoneAudioInput::~VirtualMicrophoneAudioInput()
{
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::Init()
{
	AMF_RESULT res = AMF_FAIL;

#ifdef WIN32
	HRESULT hr = CoInitialize(nullptr);
    m_bCoInitializeSucceeded = SUCCEEDED(hr);

	AMFCreateVirtualAudioManager_Fn pAudioFun = (AMFCreateVirtualAudioManager_Fn)amf_get_proc_address(g_AMFFactory.GetAMFDLLHandle(), AMF_CREATE_VIRTUAL_AUDIO_MANAGER_FUNCTION_NAME);
	AMF_RETURN_IF_FALSE(pAudioFun != nullptr, AMF_FAIL, L"AMFCreateVirtualAudioManager() is not availalbe in AMF DLL");
	res = pAudioFun(AMF_FULL_VERSION, nullptr, &m_pAudioManager);
	AMF_RETURN_IF_FAILED(res, L"AMFCreateVirtualAudioManager() failed");
#endif

	if (res == AMF_OK)
	{
		res = CreateVirtualAudioInput();
	}
	if (res != AMF_OK)
	{
		Terminate();
	}


	return res;
}

//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::CreateVirtualAudioInput()
{
	AMF_RESULT res = AMF_OK;
	res = m_pAudioManager->CreateInput(&m_pVirtualAudioInput);
	AMF_RETURN_IF_FAILED(res, L"CreateInput() failed");

	return AMF_OK;
}

//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::DestroyVirtualAudioInput()
{
	AMF_RESULT res = AMF_OK;
	if (nullptr != m_pVirtualAudioInput)
	{
		m_pVirtualAudioInput.Release();
	}
	AMF_RETURN_IF_FAILED(res, L"CreateInput() failed");

	return AMF_OK;
}

//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::Terminate()
{
	DestroyVirtualAudioInput();

	if (nullptr != m_pAudioManager)
	{
		m_pAudioManager.Release();
	}

#ifdef WIN32
    if (m_bCoInitializeSucceeded)
    {
        ::CoUninitialize();
        m_bCoInitializeSucceeded = false;
    }
#endif
	return AMF_OK;
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::EnableInput()
{
	AMF_RESULT res = AMF_OK;
	res = m_pVirtualAudioInput->SetStatus(amf::AMF_VAS_CONNECTED);
	AMF_RETURN_IF_FAILED(res, L"SetStatus(amf::AMF_VAS_CONNECTED) failed");
	return AMF_OK;
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::CheckStatus()
{
	amf::AMF_VIRTUAL_AUDIO_STATUS status = m_pVirtualAudioInput->GetStatus();
	switch (status)
	{
	case amf::AMF_VAS_UNKNOWN:
		AMFTraceInfo(AMF_FACILITY, L"Virtual Audio Input status: UNKNOWN");
		break;
	case amf::AMF_VAS_CONNECTED:
        AMFTraceInfo(AMF_FACILITY, L"Virtual Audio Input status: CONNECTED");
		break;
	case amf::AMF_VAS_DISCONNECTED:
        AMFTraceInfo(AMF_FACILITY, L"Virtual Audio Input status: DISCONNECTED");
		break;
	}
	return AMF_OK;
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::CheckFormat()
{
	AMF_RESULT res = AMF_OK;
	amf::AMFVirtualAudioFormat format = {};
	res = m_pVirtualAudioInput->GetFormat(&format);
	AMF_RETURN_IF_FAILED(res, L"GetFormat() failed");

    AMFTraceInfo(AMF_FACILITY, L"Virtual Audio Input format: SampleRate=%d channels=%d sampleSize=%d", format.sampleRate, format.channelCount, format.sampleSize);
	return AMF_OK;
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::ChangeInputParameters(amf_int64   sampleRate, amf_int64   channelCount, amf_int64  audioFormat)
{
	amf_int32 bytesInSample = 2;
	switch (audioFormat)
	{
	case AMFAF_U8: bytesInSample = 1; break;
	case AMFAF_S16: bytesInSample = 2; break;
	case AMFAF_S32: bytesInSample = 4; break;
	case AMFAF_FLT: bytesInSample = 4; break;
	case AMFAF_DBL: bytesInSample = 8; break;
	case AMFAF_U8P: bytesInSample = 1; break;
	case AMFAF_S16P: bytesInSample = 2; break;
	case AMFAF_S32P: bytesInSample = 4; break;
	case AMFAF_FLTP: bytesInSample = 4; break;
	case AMFAF_DBLP: bytesInSample = 8; break;
	}

	amf::AMFVirtualAudioFormat audioformat = { 
		static_cast<amf_int32>(sampleRate & 0xFFFFFFFF),
		static_cast<amf_int32>(channelCount & 0xFFFFFFFF),
		bytesInSample };

	return ChangeInputParameters(&audioformat);
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::ChangeInputParameters(amf::AMFVirtualAudioFormat* format)
{
	AMF_RESULT res = AMF_OK;
	res = m_pVirtualAudioInput->SetFormat(format);
	AMF_RETURN_IF_FAILED(res, L"SetFormat() failed");
	return AMF_OK;
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::VerifyInputParameters(amf::AMFVirtualAudioFormat* formatVer)
{
	AMF_RESULT res = AMF_OK;
	amf::AMFVirtualAudioFormat format = {};
	res = m_pVirtualAudioInput->GetFormat(&format);
	AMF_RETURN_IF_FAILED(res, L"GetFormat() failed");
	//    AMF_RETURN_IF_FALSE(format.sampleRate == formatVer->sampleRate && format.channelCount == formatVer->channelCount && format.sampleSize == formatVer->sampleSize, AMF_FAIL, L"Formats dont match" );
	if (format.sampleRate == formatVer->sampleRate && format.channelCount == formatVer->channelCount && format.sampleSize == formatVer->sampleSize)
	{
		return AMF_OK;
	}
	AMFTraceError(AMF_FACILITY, L"Formats dont match");
	return AMF_FAIL;
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::DisableInput()
{
	AMF_RESULT res = AMF_OK;
	res = m_pVirtualAudioInput->SetStatus(amf::AMF_VAS_DISCONNECTED);
	AMF_RETURN_IF_FAILED(res, L"SetStatus(amf::AMF_VAS_DISCONNECTED) failed");
	return AMF_OK;
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::SubmitInput(amf::AMFAudioBufferPtr pAudioBuffer)
{
	return SubmitData(pAudioBuffer->GetNative(), pAudioBuffer->GetSize());
}
//-------------------------------------------------------------------------------------------------
AMF_RESULT VirtualMicrophoneAudioInput::SubmitData(const void* data, amf_size sizeInBytes)
{
    AMF_RESULT res = AMF_OK;
    if (nullptr != m_pVirtualAudioInput)
    {
        return m_pVirtualAudioInput->SubmitData(data, sizeInBytes);
    }
	return res;
}
#endif // #if defined(_WIN32)