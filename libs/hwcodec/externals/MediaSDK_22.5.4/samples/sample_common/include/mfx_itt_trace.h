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

#ifndef __MFX_ITT_TRACE_H__
#define __MFX_ITT_TRACE_H__

#ifdef ITT_SUPPORT
#include <ittnotify.h>
#endif


#ifdef ITT_SUPPORT

static inline __itt_domain* mfx_itt_get_domain() {
    static __itt_domain *domain = NULL;

    if (!domain) domain = __itt_domain_create("MFX_SAMPLES");
    return domain;
}

class MFX_ITT_Tracer
{
public:
    MFX_ITT_Tracer(const char* trace_name)
    {
        m_domain = mfx_itt_get_domain();
        if (m_domain)
            __itt_task_begin(m_domain, __itt_null, __itt_null, __itt_string_handle_create(trace_name));
    }
    ~MFX_ITT_Tracer()
    {
        if (m_domain) __itt_task_end(m_domain);
    }
private:
    __itt_domain* m_domain;
};
#define MFX_ITT_TASK(x) MFX_ITT_Tracer __mfx_itt_tracer(x);

#else
#define MFX_ITT_TASK(x)
#endif

#endif //__MFX_ITT_TRACE_H__
