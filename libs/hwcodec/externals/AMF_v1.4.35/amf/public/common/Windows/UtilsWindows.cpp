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

#include "UtilsWindows.h"
#include <public/common/TraceAdapter.h>

#ifdef _WIN32

#define AMF_FACILITY L"UtilWindows"

amf_bool OSIsVersionOrGreater(DWORD major, DWORD minor, DWORD build)
{
    // This function is to replace "IsWindowsVersionOrGreater" since
    // it seems to be broken since windows 8

    HMODULE hModule = GetModuleHandleW(L"ntdll");

    if (hModule == nullptr)
    {
        AMFTraceError(AMF_FACILITY, L"OSIsVersionOrGreater() - Could not get ntdll handle");
        return false;
    }

    NTSTATUS(WINAPI * RtlGetVersion)(LPOSVERSIONINFOEXW);
    *(FARPROC*)&RtlGetVersion = GetProcAddress(hModule, "RtlGetVersion");

    if (RtlGetVersion == nullptr)
    {
        AMFTraceError(AMF_FACILITY, L"OSIsVersionOrGreater() - Could not get RtlGetVersion procedure handle from ntdll");
        return false;
    }

    OSVERSIONINFOEXW osVersionInfo = {};
    NTSTATUS status = RtlGetVersion(&osVersionInfo);

    if (status != 0)
    {
        AMFTraceError(AMF_FACILITY, L"OSIsVersionOrGreater() - RtlGetVersion failed");
        return false;
    }

    const DWORD versions[] = { osVersionInfo.dwMajorVersion, osVersionInfo.dwMinorVersion, osVersionInfo.dwBuildNumber };
    const DWORD checkVersions[] = { major, minor, build };
    constexpr amf_uint count = amf_countof(versions);

    for (amf_uint i = 0; i < count; ++i)
    {
        if (versions[i] == checkVersions[i])
        {
            continue;
        }

        return  versions[i] > checkVersions[i];
    }

    return versions[count - 1] == checkVersions[count - 1];
}

AMF_RESULT GetDisplayInfo(amf_handle hwnd, DisplayInfo& displayInfo)
{
    AMF_RETURN_IF_FALSE(hwnd != nullptr, AMF_INVALID_ARG, L"GetDisplayInfoFromWindow() - hwnd is NULL");
    displayInfo = {};

    // Get display information (specifically name)
    displayInfo.hMonitor = MonitorFromWindow((HWND)hwnd, MONITOR_DEFAULTTONEAREST);
    AMF_RETURN_IF_FALSE(displayInfo.hMonitor != nullptr, AMF_FAIL, L"GetDisplayInfoFromWindow() - MonitorFromWindow() returned NULL");

    MONITORINFOEX monitorInfo = {};
    monitorInfo.cbSize = sizeof(monitorInfo);
    BOOL ret = GetMonitorInfoW(displayInfo.hMonitor, &monitorInfo);
    AMF_RETURN_IF_FALSE(ret != 0, AMF_FAIL, L"GetDisplayInfoFromWindow() - GetMonitorInfoW() failed, code=%d", ret);

    // Save the windowed DEVMODE
    DEVMODEW devMode = {};
    devMode.dmSize = sizeof(devMode);
    ret = EnumDisplaySettingsW(monitorInfo.szDevice, ENUM_CURRENT_SETTINGS, &devMode);
    AMF_RETURN_IF_FALSE(ret != 0, AMF_FAIL, L"GetDisplayInfoFromWindow() - EnumDisplaySettingsW() failed to get Window Mode DEVMODE, code=%d", ret);

    displayInfo.deviceName = monitorInfo.szDevice;
    displayInfo.primary = (monitorInfo.dwFlags & MONITORINFOF_PRIMARY) == MONITORINFOF_PRIMARY;
    displayInfo.workRect = AMFConstructRect(monitorInfo.rcWork.left, monitorInfo.rcWork.top, monitorInfo.rcWork.right, monitorInfo.rcWork.bottom);
    displayInfo.monitorRect = AMFConstructRect(monitorInfo.rcMonitor.left, monitorInfo.rcMonitor.top, monitorInfo.rcMonitor.right, monitorInfo.rcMonitor.bottom);
    displayInfo.deviceRect = AMFConstructRect(devMode.dmPosition.x, devMode.dmPosition.y, devMode.dmPosition.x + devMode.dmPelsWidth, devMode.dmPosition.y + devMode.dmPelsHeight);
    displayInfo.frequency = AMFConstructRate(devMode.dmDisplayFrequency, 1);
    displayInfo.bitsPerPixel = devMode.dmBitsPerPel;
  
    return AMF_OK;
}

#endif // _WIN32