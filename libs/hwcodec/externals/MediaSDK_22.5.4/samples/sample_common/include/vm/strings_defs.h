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

#ifndef __STRING_DEFS_H__
#define __STRING_DEFS_H__

#include <stdio.h>
#include <stdlib.h>
#include <memory.h>

#ifdef __cplusplus
#include <string>
#endif

#if defined(_WIN32) || defined(_WIN64)

#include <tchar.h>

#define MSDK_STRING(x) _T(x)
#define MSDK_CHAR(x) _T(x)

#ifdef __cplusplus
typedef std::basic_string<TCHAR> msdk_tstring;
#endif
typedef TCHAR msdk_char;

#define msdk_printf   _tprintf
#define msdk_fprintf  _ftprintf
#define msdk_sprintf  _stprintf_s // to be removed
#define msdk_vprintf  _vtprintf
#define msdk_strlen   _tcslen
#define msdk_strcmp   _tcscmp
#define msdk_stricmp  _tcsicmp
#define msdk_strncmp  _tcsnicmp
#define msdk_strstr   _tcsstr
#define msdk_atoi     _ttoi
#define msdk_strtol   _tcstol
#define msdk_strtod   _tcstod
#define msdk_strchr   _tcschr
#define msdk_strnlen(str,lenmax) strnlen_s(str,lenmax)
#define msdk_sscanf _stscanf_s
#define msdk_itoa_decimal(value, str) _itot_s(value,str,10)

// msdk_strcopy is intended to be used with 2 parmeters, i.e. msdk_strcopy(dst, src)
// for _tcscpy_s that's possible if DST is declared as: TCHAR DST[n];
#define msdk_strcopy _tcscpy_s
#define msdk_strncopy_s _tcsncpy_s

#define MSDK_MEMCPY_BITSTREAM(bitstream, offset, src, count) memcpy_s((bitstream).Data + (offset), (bitstream).MaxLength - (offset), (src), (count))

#define MSDK_MEMCPY_BUF(bufptr, offset, maxsize, src, count) memcpy_s((bufptr)+ (offset), (maxsize) - (offset), (src), (count))

#define MSDK_MEMCPY_VAR(dstVarName, src, count) memcpy_s(&(dstVarName), sizeof(dstVarName), (src), (count))

#define MSDK_MEMCPY(dst, src, count) memcpy_s(dst, (count), (src), (count))



#else // #if defined(_WIN32) || defined(_WIN64)

#define MSDK_STRING(x)  x
#define MSDK_CHAR(x) x

#ifdef __cplusplus
typedef std::string msdk_tstring;
#endif
typedef char msdk_char;

#define msdk_printf   printf
#define msdk_sprintf  sprintf
#define msdk_vprintf  vprintf
#define msdk_fprintf  fprintf
#define msdk_strlen   strlen
#define msdk_strcmp   strcmp
#define msdk_stricmp  strcasecmp
#define msdk_strncmp  strncmp
#define msdk_strstr   strstr
#define msdk_atoi     atoi
#define msdk_atoll    atoll
#define msdk_strtol   strtol
#define msdk_strtod   strtod
#define msdk_itoa_decimal(value, str) \
  snprintf(str, sizeof(str)/sizeof(str[0])-1, "%d", value)
#define msdk_strnlen(str,maxlen) strlen(str)
#define msdk_sscanf sscanf

#define msdk_strcopy strcpy

#define msdk_strncopy_s(dst, num_dst, src, count) strncpy(dst, src, count)

#define MSDK_MEMCPY_BITSTREAM(bitstream, offset, src, count) memcpy((bitstream).Data + (offset), (src), (count))

#define MSDK_MEMCPY_BUF(bufptr, offset, maxsize, src, count) memcpy((bufptr)+ (offset), (src), (count))

#define MSDK_MEMCPY_VAR(dstVarName, src, count) memcpy(&(dstVarName), (src), (count))

#define MSDK_MEMCPY(dst, src, count) memcpy(dst, (src), (count))

#endif // #if defined(_WIN32) || defined(_WIN64)

#endif //__STRING_DEFS_H__
