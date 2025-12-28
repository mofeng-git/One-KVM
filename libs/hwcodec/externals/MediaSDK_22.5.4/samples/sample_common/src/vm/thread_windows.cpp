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

#include "mfx_samples_config.h"

#if defined(_WIN32) || defined(_WIN64)

#include "vm/thread_defs.h"
#include <new>

MSDKSemaphore::MSDKSemaphore(mfxStatus &sts, mfxU32 count)
{
    sts = MFX_ERR_NONE;
    m_semaphore = CreateSemaphore(NULL, count, LONG_MAX, 0);
    if (!m_semaphore) throw std::bad_alloc();
}

MSDKSemaphore::~MSDKSemaphore(void)
{
    CloseHandle(m_semaphore);
}

mfxStatus MSDKSemaphore::Post(void)
{
    return (ReleaseSemaphore(m_semaphore, 1, NULL) == false) ? MFX_ERR_UNKNOWN : MFX_ERR_NONE;
}

mfxStatus MSDKSemaphore::Wait(void)
{
    return (WaitForSingleObject(m_semaphore, INFINITE) != WAIT_OBJECT_0) ? MFX_ERR_UNKNOWN : MFX_ERR_NONE;
}

/* ****************************************************************************** */

MSDKEvent::MSDKEvent(mfxStatus &sts, bool manual, bool state)
{
    sts = MFX_ERR_NONE;
    m_event = CreateEvent(NULL, manual, state, NULL);
    if (!m_event) throw std::bad_alloc();
}

MSDKEvent::~MSDKEvent(void)
{
    CloseHandle(m_event);
}

mfxStatus MSDKEvent::Signal(void)
{
    return (SetEvent(m_event) == false) ? MFX_ERR_UNKNOWN : MFX_ERR_NONE;
}

mfxStatus MSDKEvent::Reset(void)
{
    return (ResetEvent(m_event) == false) ? MFX_ERR_UNKNOWN : MFX_ERR_NONE;
}

mfxStatus MSDKEvent::Wait(void)
{
    return (WaitForSingleObject(m_event, INFINITE) != WAIT_OBJECT_0) ? MFX_ERR_UNKNOWN : MFX_ERR_NONE;
}

mfxStatus MSDKEvent::TimedWait(mfxU32 msec)
{
    if(MFX_INFINITE == msec) return MFX_ERR_UNSUPPORTED;
    mfxStatus mfx_res = MFX_ERR_NOT_INITIALIZED;
    DWORD res = WaitForSingleObject(m_event, msec);

    if(WAIT_OBJECT_0 == res) mfx_res = MFX_ERR_NONE;
    else if (WAIT_TIMEOUT == res) mfx_res = MFX_TASK_WORKING;
    else mfx_res = MFX_ERR_UNKNOWN;

    return mfx_res;
}

MSDKThread::MSDKThread(mfxStatus &sts, msdk_thread_callback func, void* arg)
{
    sts = MFX_ERR_NONE;
    m_thread = (void*)_beginthreadex(NULL, 0, func, arg, 0, NULL);
    if (!m_thread) throw std::bad_alloc();
}

MSDKThread::~MSDKThread(void)
{
    CloseHandle(m_thread);
}

mfxStatus MSDKThread::Wait(void)
{
    return (WaitForSingleObject(m_thread, INFINITE) != WAIT_OBJECT_0) ? MFX_ERR_UNKNOWN : MFX_ERR_NONE;
}

mfxStatus MSDKThread::TimedWait(mfxU32 msec)
{
    if(MFX_INFINITE == msec) return MFX_ERR_UNSUPPORTED;

    mfxStatus mfx_res = MFX_ERR_NONE;
    DWORD res = WaitForSingleObject(m_thread, msec);

    if(WAIT_OBJECT_0 == res) mfx_res = MFX_ERR_NONE;
    else if (WAIT_TIMEOUT == res) mfx_res = MFX_TASK_WORKING;
    else mfx_res = MFX_ERR_UNKNOWN;

    return mfx_res;
}

mfxStatus MSDKThread::GetExitCode()
{
    mfxStatus mfx_res = MFX_ERR_NOT_INITIALIZED;

    DWORD code = 0;
    int sts = 0;
    sts = GetExitCodeThread(m_thread, &code);

    if (sts == 0) mfx_res = MFX_ERR_UNKNOWN;
    else if (STILL_ACTIVE == code) mfx_res = MFX_TASK_WORKING;
    else mfx_res = MFX_ERR_NONE;

    return mfx_res;
}

mfxU32 msdk_get_current_pid()
{
    return GetCurrentProcessId();
}

#endif // #if defined(_WIN32) || defined(_WIN64)
