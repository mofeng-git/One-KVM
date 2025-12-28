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

#if !defined(_WIN32) && !defined(_WIN64)

#include <new> // std::bad_alloc
#include <stdio.h> // setrlimit
#include <sched.h>
#include <unistd.h>
#include <sys/syscall.h>

#include "vm/thread_defs.h"
#include "sample_utils.h"

/* ****************************************************************************** */

MSDKSemaphore::MSDKSemaphore(mfxStatus &sts, mfxU32 count):
    msdkSemaphoreHandle(count)
{
    sts = MFX_ERR_NONE;
    int res = pthread_cond_init(&m_semaphore, NULL);
    if (!res) {
        res = pthread_mutex_init(&m_mutex, NULL);
        if (res) {
            pthread_cond_destroy(&m_semaphore);
        }
    }
    if (res) throw std::bad_alloc();
}

MSDKSemaphore::~MSDKSemaphore(void)
{
    pthread_mutex_destroy(&m_mutex);
    pthread_cond_destroy(&m_semaphore);
}

mfxStatus MSDKSemaphore::Post(void)
{
    int res = pthread_mutex_lock(&m_mutex);
    if (!res) {
        if (0 == m_count++) res = pthread_cond_signal(&m_semaphore);
    }
    int sts = pthread_mutex_unlock(&m_mutex);
    if (!res) res = sts;
    return (res)? MFX_ERR_UNKNOWN: MFX_ERR_NONE;
}

mfxStatus MSDKSemaphore::Wait(void)
{
    int res = pthread_mutex_lock(&m_mutex);
    if (!res) {
        while(!m_count) {
            res = pthread_cond_wait(&m_semaphore, &m_mutex);
        }
        if (!res) --m_count;
        int sts = pthread_mutex_unlock(&m_mutex);
        if (!res) res = sts;
    }
    return (res)? MFX_ERR_UNKNOWN: MFX_ERR_NONE;
}

/* ****************************************************************************** */

MSDKEvent::MSDKEvent(mfxStatus &sts, bool manual, bool state):
    msdkEventHandle(manual, state)
{
    sts = MFX_ERR_NONE;

    int res = pthread_cond_init(&m_event, NULL);
    if (!res) {
        res = pthread_mutex_init(&m_mutex, NULL);
        if (res) {
            pthread_cond_destroy(&m_event);
        }
    }
    if (res) throw std::bad_alloc();
}

MSDKEvent::~MSDKEvent(void)
{
    pthread_mutex_destroy(&m_mutex);
    pthread_cond_destroy(&m_event);
}

mfxStatus MSDKEvent::Signal(void)
{
    int res = pthread_mutex_lock(&m_mutex);
    if (!res) {
        if (!m_state) {
            m_state = true;
            if (m_manual) res = pthread_cond_broadcast(&m_event);
            else res = pthread_cond_signal(&m_event);
        }
        int sts = pthread_mutex_unlock(&m_mutex);
        if (!res) res = sts;
    }
    return (res)? MFX_ERR_UNKNOWN: MFX_ERR_NONE;
}

mfxStatus MSDKEvent::Reset(void)
{
    int res = pthread_mutex_lock(&m_mutex);
    if (!res)
    {
        if (m_state) m_state = false;
        res = pthread_mutex_unlock(&m_mutex);
    }
    return (res)? MFX_ERR_UNKNOWN: MFX_ERR_NONE;
}

mfxStatus MSDKEvent::Wait(void)
{
    int res = pthread_mutex_lock(&m_mutex);
    if (!res)
    {
        while(!m_state) res = pthread_cond_wait(&m_event, &m_mutex);
        if (!m_manual) m_state = false;
        int sts = pthread_mutex_unlock(&m_mutex);
        if (!res) res = sts;
    }
    return (res)? MFX_ERR_UNKNOWN: MFX_ERR_NONE;
}

mfxStatus MSDKEvent::TimedWait(mfxU32 msec)
{
    if (MFX_INFINITE == msec) return MFX_ERR_UNSUPPORTED;
    mfxStatus mfx_res = MFX_ERR_NOT_INITIALIZED;

    int res = pthread_mutex_lock(&m_mutex);
    if (!res)
    {
        if (!m_state)
        {
            struct timeval tval;
            struct timespec tspec;
            mfxI32 res;

            gettimeofday(&tval, NULL);
            msec = 1000 * msec + tval.tv_usec;
            tspec.tv_sec = tval.tv_sec + msec / 1000000;
            tspec.tv_nsec = (msec % 1000000) * 1000;
            res = pthread_cond_timedwait(&m_event,
                                         &m_mutex,
                                         &tspec);
            if (!res) mfx_res = MFX_ERR_NONE;
            else if (ETIMEDOUT == res) mfx_res = MFX_TASK_WORKING;
            else mfx_res = MFX_ERR_UNKNOWN;
        }
        else mfx_res = MFX_ERR_NONE;
        if (!m_manual)
            m_state = false;

        res = pthread_mutex_unlock(&m_mutex);
        if (res) mfx_res = MFX_ERR_UNKNOWN;
    }
    else mfx_res = MFX_ERR_UNKNOWN;

    return mfx_res;
}

/* ****************************************************************************** */

void* msdk_thread_start(void* arg)
{
    if (arg) {
        MSDKThread* thread = (MSDKThread*)arg;

        if (thread->m_func) thread->m_func(thread->m_arg);
        thread->m_event->Signal();
    }
    return NULL;
}

/* ****************************************************************************** */

MSDKThread::MSDKThread(mfxStatus &sts, msdk_thread_callback func, void* arg):
    msdkThreadHandle(func, arg)
{
    m_event = new MSDKEvent(sts, false, false);
    if (pthread_create(&(m_thread), NULL, msdk_thread_start, this)) {
        delete(m_event);
        throw std::bad_alloc();
    }
}

MSDKThread::~MSDKThread(void)
{
    delete m_event;
}

mfxStatus MSDKThread::Wait(void)
{
    int res = pthread_join(m_thread, NULL);
    return (res)? MFX_ERR_UNKNOWN: MFX_ERR_NONE;
}

mfxStatus MSDKThread::TimedWait(mfxU32 msec)
{
    if (MFX_INFINITE == msec) return MFX_ERR_UNSUPPORTED;

    mfxStatus mfx_res = m_event->TimedWait(msec);

    if (MFX_ERR_NONE == mfx_res) {
        return (pthread_join(m_thread, NULL))? MFX_ERR_UNKNOWN: MFX_ERR_NONE;
    }
    return mfx_res;
}

mfxStatus MSDKThread::GetExitCode()
{
    if (!m_event) return MFX_ERR_NOT_INITIALIZED;

    /** @todo: Need to add implementation. */
    return MFX_ERR_NONE;
}

/* ****************************************************************************** */

mfxStatus msdk_setrlimit_vmem(mfxU64 size)
{
    struct rlimit limit;

    limit.rlim_cur = size;
    limit.rlim_max = size;
    if (setrlimit(RLIMIT_AS, &limit)) return MFX_ERR_UNKNOWN;
    return MFX_ERR_NONE;
}

mfxStatus msdk_thread_get_schedtype(const msdk_char* str, mfxI32 &type)
{
    if (!msdk_strcmp(str, MSDK_STRING("fifo"))) {
        type = SCHED_FIFO;
    }
    else if (!msdk_strcmp(str, MSDK_STRING("rr"))) {
        type = SCHED_RR;
    }
    else if (!msdk_strcmp(str, MSDK_STRING("other"))) {
        type = SCHED_OTHER;
    }
    else if (!msdk_strcmp(str, MSDK_STRING("batch"))) {
        type = SCHED_BATCH;
    }
    else if (!msdk_strcmp(str, MSDK_STRING("idle"))) {
        type = SCHED_IDLE;
    }
//    else if (!msdk_strcmp(str, MSDK_STRING("deadline"))) {
//        type = SCHED_DEADLINE;
//    }
    else {
        return MFX_ERR_UNSUPPORTED;
    }
    return MFX_ERR_NONE;
}

void msdk_thread_printf_scheduling_help()
{
    msdk_printf(MSDK_STRING("Note on the scheduling types and priorities:\n"));
    msdk_printf(MSDK_STRING("  - <sched_type>: <priority_min> .. <priority_max> (notes)\n"));
    msdk_printf(MSDK_STRING("The following scheduling types requires root privileges:\n"));
    msdk_printf(MSDK_STRING("  - fifo: %d .. %d (static priority: low .. high)\n"), sched_get_priority_min(SCHED_FIFO), sched_get_priority_max(SCHED_FIFO));
    msdk_printf(MSDK_STRING("  - rr: %d .. %d (static priority: low .. high)\n"), sched_get_priority_min(SCHED_RR), sched_get_priority_max(SCHED_RR));
    msdk_printf(MSDK_STRING("The following scheduling types can be used by non-privileged users:\n"));
    msdk_printf(MSDK_STRING("  - other: 0 .. 0 (static priority always 0)\n"));
    msdk_printf(MSDK_STRING("  - batch: 0 .. 0 (static priority always 0)\n"));
    msdk_printf(MSDK_STRING("  - idle: n/a\n"));
    msdk_printf(MSDK_STRING("If you want to adjust priority for the other or batch scheduling type,\n"));
    msdk_printf(MSDK_STRING("you can do that process-wise using dynamic priority - so called nice value.\n"));
    msdk_printf(MSDK_STRING("Range for the nice value is: %d .. %d (high .. low)\n"), PRIO_MIN, PRIO_MAX);
    msdk_printf(MSDK_STRING("Please, see 'man(1) nice' for details.\n"));
}

mfxU32 msdk_get_current_pid()
{
    return syscall(SYS_getpid);
}

#endif // #if !defined(_WIN32) && !defined(_WIN64)
