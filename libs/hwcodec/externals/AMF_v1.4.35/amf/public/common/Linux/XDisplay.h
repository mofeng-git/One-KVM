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
#include <memory>
#include <X11/X.h>
#include <X11/Xlib.h>

//this pattern makes it impossible to use the x11 Display* pointer without first calling XLockDisplay
class XDisplay {
public:
    typedef std::shared_ptr<XDisplay> Ptr;

    XDisplay()
        : m_pDisplay(XOpenDisplay(nullptr))
        , m_shouldClose(true)
    {}
    XDisplay(Display* dpy)
        : m_pDisplay(dpy)
        , m_shouldClose(false)
    {}
    ~XDisplay() { if(IsValid() && m_shouldClose) XCloseDisplay(m_pDisplay); }

    bool IsValid() { return m_pDisplay != nullptr; }

private:
    Display* m_pDisplay;
    bool m_shouldClose = false;
    friend class XDisplayPtr;
};

class XDisplayPtr {
public:

    XDisplayPtr() = delete;
    XDisplayPtr(const XDisplayPtr&) = delete;
    XDisplayPtr& operator=(const XDisplayPtr&) =delete;

    explicit XDisplayPtr(std::shared_ptr<XDisplay> display) : m_pDisplay(display) { XLockDisplay(m_pDisplay->m_pDisplay); }
    ~XDisplayPtr() { XUnlockDisplay(m_pDisplay->m_pDisplay); }

    //XDisplayPtr acts like a normal Display* pointer, but the only way to obtain it is by locking the Display
    operator Display*() { return m_pDisplay->m_pDisplay; }

private:
    XDisplay::Ptr m_pDisplay;
};
