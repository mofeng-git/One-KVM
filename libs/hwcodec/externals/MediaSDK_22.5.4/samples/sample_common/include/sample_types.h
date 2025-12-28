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

#ifndef __SAMPLE_TYPES_H__
#define __SAMPLE_TYPES_H__

#ifdef UNICODE
    #define msdk_cout std::wcout
    #define msdk_err std::wcerr
#else
    #define msdk_cout std::cout
    #define msdk_err std::cerr
#endif

typedef std::basic_string<msdk_char> msdk_string;
typedef std::basic_stringstream<msdk_char> msdk_stringstream;
typedef std::basic_ostream<msdk_char, std::char_traits<msdk_char> > msdk_ostream;
typedef std::basic_istream<msdk_char, std::char_traits<msdk_char> > msdk_istream;
typedef std::basic_fstream<msdk_char, std::char_traits<msdk_char> > msdk_fstream;

#if defined(_UNICODE)
#define MSDK_MAKE_BYTE_STRING(src,dest) \
    {\
        std::wstring wstr(src);\
        std::string str(wstr.length(), 0);\
        std::transform(wstr.begin(), wstr.end(), str.begin(), [](wchar_t c) {\
            return (char)c;\
        });\
        strcpy_s(dest, str.c_str());\
    }
#else
#define MSDK_MAKE_BYTE_STRING(src,dest) msdk_strcopy(dest, src);
#endif


#endif //__SAMPLE_TYPES_H__
