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

#ifndef __THREAD_DEFS_H__
#define __THREAD_DEFS_H__

#include "mfxdefs.h"
#include "vm/strings_defs.h"

typedef unsigned int (MFX_STDCALL * msdk_thread_callback)(void*);

#if defined(_WIN32) || defined(_WIN64)

#include <windows.h>
#include <process.h>

struct msdkSemaphoreHandle
{
    void* m_semaphore;
};

struct msdkEventHandle
{
    void* m_event;
};

struct msdkThreadHandle
{
    void* m_thread;
};

#else // #if defined(_WIN32) || defined(_WIN64)

#include <pthread.h>
#include <errno.h>
#include <sys/time.h>
#include <sys/resource.h>

struct msdkSemaphoreHandle
{
    msdkSemaphoreHandle(mfxU32 count):
        m_count(count)
    {}

    mfxU32 m_count;
    pthread_cond_t m_semaphore;
    pthread_mutex_t m_mutex;
};

struct msdkEventHandle
{
    msdkEventHandle(bool manual, bool state):
        m_manual(manual),
        m_state(state)
    {}

    bool m_manual;
    bool m_state;
    pthread_cond_t m_event;
    pthread_mutex_t m_mutex;
};

class MSDKEvent;

struct msdkThreadHandle
{
    msdkThreadHandle(
        msdk_thread_callback func,
        void* arg):
      m_func(func),
      m_arg(arg),
      m_event(0),
      m_thread(0)
    {}

    msdk_thread_callback m_func;
    void* m_arg;
    MSDKEvent* m_event;
    pthread_t m_thread;
};

#endif // #if defined(_WIN32) || defined(_WIN64)

class MSDKSemaphore: public msdkSemaphoreHandle
{
public:
    MSDKSemaphore(mfxStatus &sts, mfxU32 count = 0);
    ~MSDKSemaphore(void);

    mfxStatus Post(void);
    mfxStatus Wait(void);

private:
    MSDKSemaphore(const MSDKSemaphore&);
    void operator=(const MSDKSemaphore&);
};

class MSDKEvent: public msdkEventHandle
{
public:
    MSDKEvent(mfxStatus &sts, bool manual, bool state);
    ~MSDKEvent(void);

    mfxStatus Signal(void);
    mfxStatus Reset(void);
    mfxStatus Wait(void);
    mfxStatus TimedWait(mfxU32 msec);

private:
    MSDKEvent(const MSDKEvent&);
    void operator=(const MSDKEvent&);
};

class MSDKThread: public msdkThreadHandle
{
public:
    MSDKThread(mfxStatus &sts, msdk_thread_callback func, void* arg);
    ~MSDKThread(void);

    mfxStatus Wait(void);
    mfxStatus TimedWait(mfxU32 msec);
    mfxStatus GetExitCode();

#if !defined(_WIN32) && !defined(_WIN64)
    friend void* msdk_thread_start(void* arg);
#endif

private:
    MSDKThread(const MSDKThread&);
    void operator=(const MSDKThread&);
};

mfxU32 msdk_get_current_pid();
mfxStatus msdk_setrlimit_vmem(mfxU64 size);
mfxStatus msdk_thread_get_schedtype(const msdk_char*, mfxI32 &type);
void msdk_thread_printf_scheduling_help();

#endif //__THREAD_DEFS_H__
