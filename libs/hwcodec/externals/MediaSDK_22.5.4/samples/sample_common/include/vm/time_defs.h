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

#ifndef __TIME_DEFS_H__
#define __TIME_DEFS_H__

#include "mfxdefs.h"
#include "mfx_itt_trace.h"

#if defined(_WIN32) || defined(_WIN64)

#include <windows.h>

#define MSDK_SLEEP(msec) Sleep(msec)

#define MSDK_USLEEP(usec) \
{ \
    LARGE_INTEGER due; \
    due.QuadPart = -(10*(int)usec); \
    HANDLE t = CreateWaitableTimer(NULL, TRUE, NULL); \
    SetWaitableTimer(t, &due, 0, NULL, NULL, 0); \
    WaitForSingleObject(t, INFINITE); \
    CloseHandle(t); \
}

#else // #if defined(_WIN32) || defined(_WIN64)

#include <unistd.h>

#define MSDK_SLEEP(msec) \
  do { \
    MFX_ITT_TASK("MSDK_SLEEP"); \
    usleep(1000*msec); \
  } while(0)

#define MSDK_USLEEP(usec) \
  do { \
    MFX_ITT_TASK("MSDK_USLEEP"); \
    usleep(usec); \
  } while(0)

#endif // #if defined(_WIN32) || defined(_WIN64)

#define MSDK_GET_TIME(T,S,F) ((mfxF64)((T)-(S))/(mfxF64)(F))

typedef mfxI64 msdk_tick;

msdk_tick msdk_time_get_tick(void);
msdk_tick msdk_time_get_frequency(void);
mfxU64 rdtsc(void);

#endif // #ifndef __TIME_DEFS_H__
