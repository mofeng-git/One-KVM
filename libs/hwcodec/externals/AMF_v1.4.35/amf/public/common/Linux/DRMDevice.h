//
// Notice Regarding Standards.  AMD does not provide a license or sublicense to
// any Intellectual Property Rights relating to any standards, including but not
// limited to any audio and/or video codec technologies such as MPEG-2, MPEG-4;
// AVC/H.264; HEVC/H.265; AAC decode/FFMPEG; AAC encode/FFMPEG; VC-1; AV1; and MP3
// (collectively, the "Media Technologies"). For clarity, you will pay any
// royalties due for such third party technologies, which may include the Media
// Technologies that are owed as a result of AMD providing the Software to you.
//
// MIT license
//
// Copyright (c) 2023 Advanced Micro Devices, Inc. All rights reserved.
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
#pragma once
#include <string>
#include <vector>
#include "public/common/TraceAdapter.h"
#include "public/include/core/Surface.h"

#include <drm.h>
#include <drm_fourcc.h>
#include <drm_mode.h>
#include <amdgpu_drm.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

// These classed provide a nice interface to a DRM card using libdrm

amf::AMF_SURFACE_FORMAT AMF_STD_CALL FromDRMtoAMF(uint32_t formatDRM);
drmModeFB2Ptr AMF_STD_CALL AMFdrmModeGetFB2(int fd, uint32_t fb_id);
void AMF_STD_CALL AMFdrmModeFreeFB2(drmModeFB2Ptr ptr);

template  <class type, void function(type)>
class AMFAutoDRMPtr
{
    public:
    AMFAutoDRMPtr() : p(nullptr){}
    AMFAutoDRMPtr(type ptr) : p(ptr){}
    ~AMFAutoDRMPtr()
    {
        Clear();
    }
    AMFAutoDRMPtr<type, function>& operator=(type ptr)
    {
        if(p != ptr)
        {
            Clear();
            p = ptr;
        }
        return *this;
    }
    void Clear()
    {
        if(p != nullptr)
        {
            function(p);
            p = nullptr;
        }
    }

    type p;

    private:
    AMFAutoDRMPtr<type, function>& operator=(const AMFAutoDRMPtr<type, function>& other);
};

typedef   AMFAutoDRMPtr<drmModePlanePtr, drmModeFreePlane> AMFdrmModePlanePtr;
typedef   AMFAutoDRMPtr<drmModeFBPtr, drmModeFreeFB> AMFdrmModeFBPtr;
typedef   AMFAutoDRMPtr<drmModeFB2Ptr, AMFdrmModeFreeFB2> AMFdrmModeFB2Ptr;
typedef   AMFAutoDRMPtr<drmModePlaneResPtr, drmModeFreePlaneResources> AMFdrmModePlaneResPtr;
typedef   AMFAutoDRMPtr<drmModeObjectPropertiesPtr, drmModeFreeObjectProperties> AMFdrmModeObjectPropertiesPtr;
typedef   AMFAutoDRMPtr<drmModePropertyPtr, drmModeFreeProperty> AMFdrmModePropertyPtr;
typedef   AMFAutoDRMPtr<drmModeCrtcPtr, drmModeFreeCrtc> AMFdrmModeCrtcPtr;
typedef   AMFAutoDRMPtr<drmModeResPtr, drmModeFreeResources> AMFdrmModeResPtr;

struct DRMCRTC {
	int crtcID;
	int fbID;
	AMFRect crop;
	int formatDRM;
	amf::AMF_SURFACE_FORMAT formatAMF;
	int handle;
};

class DRMDevice {
public:
	DRMDevice();
	~DRMDevice();

	AMF_RESULT AMF_STD_CALL InitFromVulkan(int pciDomain, int pciBus, int pciDevice, int pciFunction);
	AMF_RESULT AMF_STD_CALL InitFromPath(const char* pathToCard);

	AMF_RESULT AMF_STD_CALL Terminate();

	int AMF_STD_CALL GetFD() const;
	std::string AMF_STD_CALL GetPathToCard() const;

	AMF_RESULT AMF_STD_CALL GetCRTCs(std::vector<DRMCRTC>& crtcs) const;
	AMF_RESULT AMF_STD_CALL GetCrtcInfo(const AMFdrmModeCrtcPtr& crtc, AMFRect &crop, int& formatDRM, amf::AMF_SURFACE_FORMAT& formatAMF, int& handle) const;
private:
	AMF_RESULT SetupDevice();

	int m_fd = -1;
	std::string m_pathToCard;
};