/******************************************************************************\
Copyright (c) 2005-2020, Intel Corporation
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

#include <math.h>
#include <iostream>
#include <algorithm>
#include <map>

#include "vm/strings_defs.h"
#include "time_statistics.h"
#include "sample_defs.h"
#include "sample_utils.h"
#include "mfxcommon.h"
#include "mfxjpeg.h"
#include "mfxvp8.h"


msdk_tick CTimer::frequency = 0;
msdk_tick CTimeStatisticsReal::frequency = 0;

mfxStatus CopyBitstream2(mfxBitstream *dest, mfxBitstream *src)
{
    if (!dest || !src)
        return MFX_ERR_NULL_PTR;

    if (!dest->DataLength)
    {
        dest->DataOffset = 0;
    }
    else
    {
        memmove(dest->Data, dest->Data + dest->DataOffset, dest->DataLength);
        dest->DataOffset = 0;
    }

    if (src->DataLength > dest->MaxLength - dest->DataLength - dest->DataOffset)
        return MFX_ERR_NOT_ENOUGH_BUFFER;

    MSDK_MEMCPY_BITSTREAM(*dest, dest->DataOffset, src->Data, src->DataLength);
    dest->DataLength = src->DataLength;

    dest->DataFlag = src->DataFlag;

    //common Extended buffer will be for src and dest bit streams
    dest->EncryptedData = src->EncryptedData;

    return MFX_ERR_NONE;
}

mfxStatus GetFrameLength(mfxU16 width, mfxU16 height, mfxU32 ColorFormat, mfxU32 &length)
{
    switch (ColorFormat)
    {
    case MFX_FOURCC_NV12:
    case MFX_FOURCC_I420:
        length = 3 * width * height / 2;
        break;
    case MFX_FOURCC_YUY2:
        length = 2 * width * height;
        break;
    case MFX_FOURCC_RGB4:
        length = 4 * width * height;
        break;
    case MFX_FOURCC_P010:
        length = 3 * width * height;
        break;
    default:
        return MFX_ERR_UNSUPPORTED;
    }

    return MFX_ERR_NONE;
}

CSmplYUVReader::CSmplYUVReader()
{
    m_bInited = false;
    m_ColorFormat = MFX_FOURCC_YV12;
    shouldShift10BitsHigh = false;
}

mfxStatus CSmplYUVReader::Init(std::list<msdk_string> inputs, mfxU32 ColorFormat, bool enableShifting)
{
    Close();

    if( MFX_FOURCC_NV12 != ColorFormat &&
        MFX_FOURCC_YV12 != ColorFormat &&
        MFX_FOURCC_I420 != ColorFormat &&
        MFX_FOURCC_YUY2 != ColorFormat &&
        MFX_FOURCC_UYVY != ColorFormat &&
        MFX_FOURCC_RGB4 != ColorFormat &&
        MFX_FOURCC_BGR4 != ColorFormat &&
        MFX_FOURCC_P010 != ColorFormat &&
        MFX_FOURCC_P210 != ColorFormat &&
        MFX_FOURCC_AYUV != ColorFormat &&
        MFX_FOURCC_A2RGB10 != ColorFormat
#if (MFX_VERSION >= 1027)
        && MFX_FOURCC_Y210 != ColorFormat
        && MFX_FOURCC_Y410 != ColorFormat
#endif
#if (MFX_VERSION >= 1031)
        && MFX_FOURCC_P016 != ColorFormat
        && MFX_FOURCC_Y216 != ColorFormat
#endif
        )
    {
        return MFX_ERR_UNSUPPORTED;
    }

    if (   MFX_FOURCC_P010 == ColorFormat
        || MFX_FOURCC_P210 == ColorFormat
#if (MFX_VERSION >= 1027)
        || MFX_FOURCC_Y210 == ColorFormat
#endif
#if (MFX_VERSION >= 1031)
        || MFX_FOURCC_P016 == ColorFormat
        || MFX_FOURCC_Y216 == ColorFormat
#endif
    )
    {
        shouldShift10BitsHigh = enableShifting;
    }

    if (!inputs.size())
    {
        return MFX_ERR_UNSUPPORTED;
    }

    for (ls_iterator it = inputs.begin(); it != inputs.end(); it++)
    {
        FILE *f = 0;
        MSDK_FOPEN(f, (*it).c_str(), MSDK_STRING("rb"));
        MSDK_CHECK_POINTER(f, MFX_ERR_NULL_PTR);

        m_files.push_back(f);
    }

    m_ColorFormat = ColorFormat;

    m_bInited = true;

    return MFX_ERR_NONE;
}

CSmplYUVReader::~CSmplYUVReader()
{
    Close();
}

void CSmplYUVReader::Close()
{
    for (mfxU32 i = 0; i < m_files.size(); i++)
    {
        fclose(m_files[i]);
    }
    m_files.clear();
    m_bInited = false;
}

void CSmplYUVReader::Reset()
{
    for (mfxU32 i = 0; i < m_files.size(); i++)
    {
        fseek(m_files[i], 0, SEEK_SET);
    }
}

mfxStatus CSmplYUVReader::SkipNframesFromBeginning(mfxU16 w, mfxU16 h, mfxU32 viewId, mfxU32 nframes)
{
    // change file position for read from beginning to "frameLength * nframes".
    mfxU32 frameLength;

    if (MFX_ERR_NONE != GetFrameLength(w, h, m_ColorFormat, frameLength))
    {
        msdk_printf(MSDK_STRING("Input color format %s is unsupported in qpfile mode\n"), ColorFormatToStr(m_ColorFormat));
        return MFX_ERR_UNSUPPORTED;
    }

    if (0 != fseek(m_files[viewId], frameLength * nframes, SEEK_SET))
        return MFX_ERR_MORE_DATA;

    return MFX_ERR_NONE;
}

mfxStatus CSmplYUVReader::LoadNextFrame(mfxFrameSurface1* pSurface)
{
    // check if reader is initialized
    MSDK_CHECK_ERROR(m_bInited, false, MFX_ERR_NOT_INITIALIZED);
    MSDK_CHECK_POINTER(pSurface, MFX_ERR_NULL_PTR);

    mfxU32 nBytesRead;
    mfxU16 w, h, i, pitch;
    mfxU8 *ptr, *ptr2;
    mfxFrameInfo& pInfo = pSurface->Info;
    mfxFrameData& pData = pSurface->Data;

    mfxU32 shiftSizeLuma   = 16 - pInfo.BitDepthLuma;
    mfxU32 shiftSizeChroma = 16 - pInfo.BitDepthChroma;

    mfxU32 vid = pInfo.FrameId.ViewId;

    if (vid > m_files.size())
    {
        return MFX_ERR_UNSUPPORTED;
    }

    if (pInfo.CropH > 0 && pInfo.CropW > 0)
    {
        w = pInfo.CropW;
        h = pInfo.CropH;
    }
    else
    {
        w = pInfo.Width;
        h = pInfo.Height;
    }

    mfxU32 nBytesPerPixel = (pInfo.FourCC == MFX_FOURCC_P010 || pInfo.FourCC == MFX_FOURCC_P210
#if (MFX_VERSION >= 1031)
        || pInfo.FourCC == MFX_FOURCC_P016
#endif
    ) ? 2 : 1;

    if (   MFX_FOURCC_YUY2 == pInfo.FourCC
        || MFX_FOURCC_UYVY == pInfo.FourCC
        || MFX_FOURCC_RGB4 == pInfo.FourCC
        || MFX_FOURCC_BGR4 == pInfo.FourCC
        || MFX_FOURCC_AYUV == pInfo.FourCC
        || MFX_FOURCC_A2RGB10 == pInfo.FourCC
#if (MFX_VERSION >= 1027)
        || MFX_FOURCC_Y210 == pInfo.FourCC
        || MFX_FOURCC_Y410 == pInfo.FourCC
#endif
#if (MFX_VERSION >= 1031)
        || MFX_FOURCC_Y216 == pInfo.FourCC
#endif
    )
    {
        //Packed format: Luminance and chrominance are on the same plane
        switch (m_ColorFormat)
        {
        case MFX_FOURCC_A2RGB10:
        case MFX_FOURCC_RGB4:
        case MFX_FOURCC_BGR4:
            pitch = pData.Pitch;
            ptr = std::min({pData.R, pData.G, pData.B});
            ptr = ptr + pInfo.CropX*4 + pInfo.CropY * pData.Pitch;

            for(i = 0; i < h; i++)
            {
                nBytesRead = (mfxU32)fread(ptr + i * pitch, 1, 4*w, m_files[vid]);

                if ((mfxU32)4*w != nBytesRead)
                {
                    return MFX_ERR_MORE_DATA;
                }
            }
            break;
        case MFX_FOURCC_YUY2:
        case MFX_FOURCC_UYVY:
            pitch = pData.Pitch;
            ptr = m_ColorFormat == MFX_FOURCC_YUY2?
                  pData.Y + pInfo.CropX*2 + pInfo.CropY * pData.Pitch
                : pData.U + pInfo.CropX   + pInfo.CropY * pData.Pitch;

            for(i = 0; i < h; i++)
            {
                nBytesRead = (mfxU32)fread(ptr + i * pitch, 2, w, m_files[vid]);

                if ((mfxU32)w != nBytesRead)
                {
                    return MFX_ERR_MORE_DATA;
                }
            }
            break;
        case MFX_FOURCC_AYUV:
            pitch = pData.Pitch;
            ptr = pData.V + pInfo.CropX*4 + pInfo.CropY * pData.Pitch;

            for (i = 0; i < h; i++)
            {
                nBytesRead = (mfxU32)fread(ptr + i * pitch, 4, w, m_files[vid]);

                if ((mfxU32)w != nBytesRead)
                {
                    return MFX_ERR_MORE_DATA;
                }
            }
            break;

#if (MFX_VERSION >= 1027)
        case MFX_FOURCC_Y210:
        case MFX_FOURCC_Y410:
#if (MFX_VERSION >= 1031)
        case MFX_FOURCC_Y216:
#endif
            pitch = pData.Pitch;
            ptr = ((pInfo.FourCC == MFX_FOURCC_Y210
#if (MFX_VERSION >= 1031)
            || pInfo.FourCC == MFX_FOURCC_Y216
#endif
            )  ? pData.Y : (mfxU8*)pData.Y410) + pInfo.CropX*4 + pInfo.CropY * pData.Pitch;

            for (i = 0; i < h; i++)
            {
                nBytesRead = (mfxU32)fread(ptr + i * pitch, 1, 4 * w, m_files[vid]);

                if ((mfxU32)4 * w != nBytesRead)
                {
                    return MFX_ERR_MORE_DATA;
                }

                if ((MFX_FOURCC_Y210 == pInfo.FourCC
#if (MFX_VERSION >= 1031)
                    || MFX_FOURCC_Y216 == pInfo.FourCC
#endif
                ) && shouldShift10BitsHigh)
                {
                    mfxU16* shortPtr = (mfxU16*)(ptr + i * pitch);
                    for (int idx = 0; idx < w*2; idx++)
                    {
                        shortPtr[idx] <<= shiftSizeLuma;
                    }
                }

            }
            break;
#endif
        default:
            return MFX_ERR_UNSUPPORTED;
        }
    }
    else if (MFX_FOURCC_NV12 == pInfo.FourCC
            || MFX_FOURCC_YV12 == pInfo.FourCC
            || MFX_FOURCC_P010 == pInfo.FourCC
            || MFX_FOURCC_P210 == pInfo.FourCC
#if (MFX_VERSION >= 1031)
            || MFX_FOURCC_P016 == pInfo.FourCC
#endif
    )
    {
        pitch = pData.Pitch;
        ptr = pData.Y + pInfo.CropX + pInfo.CropY * pData.Pitch;

        // read luminance plane
        for(i = 0; i < h; i++)
        {
            nBytesRead = (mfxU32)fread(ptr + i * pitch, nBytesPerPixel, w, m_files[vid]);

            if (w != nBytesRead)
            {
                return MFX_ERR_MORE_DATA;
            }

            // Shifting data if required
            if((MFX_FOURCC_P010 == pInfo.FourCC
             || MFX_FOURCC_P210 == pInfo.FourCC
#if (MFX_VERSION >= 1031)
             || MFX_FOURCC_P016 == pInfo.FourCC
#endif
            ) && shouldShift10BitsHigh)
            {
                mfxU16* shortPtr = (mfxU16*)(ptr + i * pitch);
                for(int idx = 0; idx < w; idx++)
                {
                    shortPtr[idx]<<=shiftSizeLuma;
                }
            }
        }

        // read chroma planes
        switch (m_ColorFormat) // color format of data in the input file
        {
        case MFX_FOURCC_I420:
        case MFX_FOURCC_YV12:
            switch (pInfo.FourCC)
            {
            case MFX_FOURCC_NV12:

                mfxU8 buf[2048]; // maximum supported chroma width for nv12
                mfxU32 j, dstOffset[2];
                w /= 2;
                h /= 2;
                ptr = pData.UV + pInfo.CropX + (pInfo.CropY / 2) * pitch;
                if (w > 2048)
                {
                    return MFX_ERR_UNSUPPORTED;
                }

                if (m_ColorFormat == MFX_FOURCC_I420) {
                    dstOffset[0] = 0;
                    dstOffset[1] = 1;
                } else {
                    dstOffset[0] = 1;
                    dstOffset[1] = 0;
                }

                // load first chroma plane: U (input == I420) or V (input == YV12)
                for (i = 0; i < h; i++)
                {
                    nBytesRead = (mfxU32)fread(buf, 1, w, m_files[vid]);
                    if (w != nBytesRead)
                    {
                        return MFX_ERR_MORE_DATA;
                    }
                    for (j = 0; j < w; j++)
                    {
                        ptr[i * pitch + j * 2 + dstOffset[0]] = buf[j];
                    }
                }

                // load second chroma plane: V (input == I420) or U (input == YV12)
                for (i = 0; i < h; i++)
                {

                    nBytesRead = (mfxU32)fread(buf, 1, w, m_files[vid]);

                    if (w != nBytesRead)
                    {
                        return MFX_ERR_MORE_DATA;
                    }
                    for (j = 0; j < w; j++)
                    {
                        ptr[i * pitch + j * 2 + dstOffset[1]] = buf[j];
                    }
                }

                break;
            case MFX_FOURCC_YV12:
                w /= 2;
                h /= 2;
                pitch /= 2;

                if (m_ColorFormat == MFX_FOURCC_I420) {
                    ptr  = pData.U + (pInfo.CropX / 2) + (pInfo.CropY / 2) * pitch;
                    ptr2 = pData.V + (pInfo.CropX / 2) + (pInfo.CropY / 2) * pitch;
                } else {
                    ptr  = pData.V + (pInfo.CropX / 2) + (pInfo.CropY / 2) * pitch;
                    ptr2 = pData.U + (pInfo.CropX / 2) + (pInfo.CropY / 2) * pitch;
                }

                for(i = 0; i < h; i++)
                {

                    nBytesRead = (mfxU32)fread(ptr + i * pitch, 1, w, m_files[vid]);

                    if (w != nBytesRead)
                    {
                        return MFX_ERR_MORE_DATA;
                    }
                }
                for(i = 0; i < h; i++)
                {
                    nBytesRead = (mfxU32)fread(ptr2 + i * pitch, 1, w, m_files[vid]);

                    if (w != nBytesRead)
                    {
                        return MFX_ERR_MORE_DATA;
                    }
                }
                break;
            default:
                return MFX_ERR_UNSUPPORTED;
            }
            break;
        case MFX_FOURCC_NV12:
        case MFX_FOURCC_P010:
        case MFX_FOURCC_P210:
#if (MFX_VERSION >= 1031)
        case MFX_FOURCC_P016:
#endif
            if (MFX_FOURCC_P210 != pInfo.FourCC)
            {
                h /= 2;
            }
            ptr  = pData.UV + pInfo.CropX + (pInfo.CropY / 2) * pitch;
            for(i = 0; i < h; i++)
            {
                nBytesRead = (mfxU32)fread(ptr + i * pitch, nBytesPerPixel, w, m_files[vid]);

                if (w != nBytesRead)
                {
                    return MFX_ERR_MORE_DATA;
                }

                // Shifting data if required
            if((MFX_FOURCC_P010 == pInfo.FourCC
             || MFX_FOURCC_P210 == pInfo.FourCC
#if (MFX_VERSION >= 1031)
             || MFX_FOURCC_P016 == pInfo.FourCC
#endif
             ) && shouldShift10BitsHigh)
                {
                    mfxU16* shortPtr = (mfxU16*)(ptr + i * pitch);
                    for(int idx = 0; idx < w; idx++)
                    {
                        shortPtr[idx]<<=shiftSizeChroma;
                    }
                }
            }

            break;
        default:
            return MFX_ERR_UNSUPPORTED;
        }
    }
    else
    {
        return MFX_ERR_UNSUPPORTED;
    }

    return MFX_ERR_NONE;
}

CSmplBitstreamWriter::CSmplBitstreamWriter()
{
    m_fSource = NULL;
    m_bInited = false;
    m_nProcessedFramesNum = 0;
}

CSmplBitstreamWriter::~CSmplBitstreamWriter()
{
    Close();
}

void CSmplBitstreamWriter::Close()
{
    if (m_fSource)
    {
        fclose(m_fSource);
        m_fSource = NULL;
    }

    m_bInited = false;
}

mfxStatus CSmplBitstreamWriter::Init(const msdk_char *strFileName)
{
    MSDK_CHECK_POINTER(strFileName, MFX_ERR_NULL_PTR);
    if (!msdk_strlen(strFileName))
        return MFX_ERR_NONE;

    Close();

    //init file to write encoded data
    MSDK_FOPEN(m_fSource, strFileName, MSDK_STRING("wb+"));
    MSDK_CHECK_POINTER(m_fSource, MFX_ERR_NULL_PTR);

    m_sFile = msdk_string(strFileName);
    //set init state to true in case of success
    m_bInited = true;
    return MFX_ERR_NONE;
}

mfxStatus CSmplBitstreamWriter::Reset()
{
    return Init(m_sFile.c_str());
}

mfxStatus CSmplBitstreamWriter::WriteNextFrame(mfxBitstream *pMfxBitstream, bool isPrint)
{
    // check if writer is initialized
    MSDK_CHECK_ERROR(m_bInited, false, MFX_ERR_NOT_INITIALIZED);
    MSDK_CHECK_POINTER(pMfxBitstream, MFX_ERR_NULL_PTR);

    mfxU32 nBytesWritten = 0;

    nBytesWritten = (mfxU32)fwrite(pMfxBitstream->Data + pMfxBitstream->DataOffset, 1, pMfxBitstream->DataLength, m_fSource);
    MSDK_CHECK_NOT_EQUAL(nBytesWritten, pMfxBitstream->DataLength, MFX_ERR_UNDEFINED_BEHAVIOR);

    // mark that we don't need bit stream data any more
    pMfxBitstream->DataLength = 0;

    m_nProcessedFramesNum++;

    // print encoding progress to console every certain number of frames (not to affect performance too much)
    if (isPrint && (1 == m_nProcessedFramesNum  || (0 == (m_nProcessedFramesNum % 100))))
    {
        msdk_printf(MSDK_STRING("Frame number: %u\r"), m_nProcessedFramesNum);
    }

    return MFX_ERR_NONE;
}


CSmplBitstreamDuplicateWriter::CSmplBitstreamDuplicateWriter()
    : CSmplBitstreamWriter()
{
    m_fSourceDuplicate = NULL;
    m_bJoined = false;
}

mfxStatus CSmplBitstreamDuplicateWriter::InitDuplicate(const msdk_char *strFileName)
{
    MSDK_CHECK_POINTER(strFileName, MFX_ERR_NULL_PTR);
    MSDK_CHECK_ERROR(msdk_strlen(strFileName), 0, MFX_ERR_NOT_INITIALIZED);

    if (m_fSourceDuplicate)
    {
        fclose(m_fSourceDuplicate);
        m_fSourceDuplicate = NULL;
    }
    MSDK_FOPEN(m_fSourceDuplicate, strFileName, MSDK_STRING("wb+"));
    MSDK_CHECK_POINTER(m_fSourceDuplicate, MFX_ERR_NULL_PTR);

    m_bJoined = false; // mark we own the file handle

    return MFX_ERR_NONE;
}

mfxStatus CSmplBitstreamDuplicateWriter::JoinDuplicate(CSmplBitstreamDuplicateWriter *pJoinee)
{
    MSDK_CHECK_POINTER(pJoinee, MFX_ERR_NULL_PTR);
    MSDK_CHECK_ERROR(pJoinee->m_fSourceDuplicate, NULL, MFX_ERR_NOT_INITIALIZED);

    m_fSourceDuplicate = pJoinee->m_fSourceDuplicate;
    m_bJoined = true; // mark we do not own the file handle

    return MFX_ERR_NONE;
}

mfxStatus CSmplBitstreamDuplicateWriter::WriteNextFrame(mfxBitstream *pMfxBitstream, bool isPrint)
{
    MSDK_CHECK_ERROR(m_fSourceDuplicate, NULL, MFX_ERR_NOT_INITIALIZED);
    MSDK_CHECK_POINTER(pMfxBitstream, MFX_ERR_NULL_PTR);

    mfxU32 nBytesWritten = (mfxU32)fwrite(pMfxBitstream->Data + pMfxBitstream->DataOffset, 1, pMfxBitstream->DataLength, m_fSourceDuplicate);
    MSDK_CHECK_NOT_EQUAL(nBytesWritten, pMfxBitstream->DataLength, MFX_ERR_UNDEFINED_BEHAVIOR);

    CSmplBitstreamWriter::WriteNextFrame(pMfxBitstream, isPrint);

    return MFX_ERR_NONE;
}

void CSmplBitstreamDuplicateWriter::Close()
{
    if (m_fSourceDuplicate && !m_bJoined)
    {
        fclose(m_fSourceDuplicate);
    }

    m_fSourceDuplicate = NULL;
    m_bJoined = false;

    CSmplBitstreamWriter::Close();
}

CSmplBitstreamReader::CSmplBitstreamReader()
{
    m_fSource = NULL;
    m_bInited = false;
}

CSmplBitstreamReader::~CSmplBitstreamReader()
{
    Close();
}

void CSmplBitstreamReader::Close()
{
    if (m_fSource)
    {
        fclose(m_fSource);
        m_fSource = NULL;
    }

    m_bInited = false;
}

void CSmplBitstreamReader::Reset()
{
    if (!m_bInited)
        return;

    fseek(m_fSource, 0, SEEK_SET);
}

mfxStatus CSmplBitstreamReader::Init(const msdk_char *strFileName)
{
    MSDK_CHECK_POINTER(strFileName, MFX_ERR_NULL_PTR);
    if (!msdk_strlen(strFileName))
        return MFX_ERR_NONE;

    Close();

    //open file to read input stream
    MSDK_FOPEN(m_fSource, strFileName, MSDK_STRING("rb"));
    MSDK_CHECK_POINTER(m_fSource, MFX_ERR_NULL_PTR);

    m_bInited = true;
    return MFX_ERR_NONE;
}

#define CHECK_SET_EOS(pBitstream)                  \
    if (feof(m_fSource))                           \
    {                                              \
        pBitstream->DataFlag |= MFX_BITSTREAM_EOS; \
    }

mfxStatus CSmplBitstreamReader::ReadNextFrame(mfxBitstream *pBS)
{
    if (!m_bInited)
        return MFX_ERR_NOT_INITIALIZED;

    MSDK_CHECK_POINTER(pBS, MFX_ERR_NULL_PTR);

    // Not enough memory to read new chunk of data
    if (pBS->MaxLength == pBS->DataLength)
        return MFX_ERR_NOT_ENOUGH_BUFFER;

    memmove(pBS->Data, pBS->Data + pBS->DataOffset, pBS->DataLength);
    pBS->DataOffset = 0;
    mfxU32 nBytesRead = (mfxU32)fread(pBS->Data + pBS->DataLength, 1, pBS->MaxLength - pBS->DataLength, m_fSource);

    CHECK_SET_EOS(pBS);

    if (0 == nBytesRead)
    {
        return MFX_ERR_MORE_DATA;
    }

    pBS->DataLength += nBytesRead;

    return MFX_ERR_NONE;
}


mfxU32 CJPEGFrameReader::FindMarker(mfxBitstream *pBS,mfxU32 startOffset,CJPEGFrameReader::JPEGMarker marker)
{
    for (mfxU32 i = startOffset; i + sizeof(mfxU16) <= pBS->DataLength; i++)
    {
        if ( *(mfxU16*)(pBS->Data + i)==(mfxU16)marker)
        {
            return i;
        }
    }
    return 0xFFFFFFFF;
}

mfxStatus CJPEGFrameReader::ReadNextFrame(mfxBitstream *pBS)
{
    mfxStatus sts = MFX_ERR_NONE;
    mfxU32 offsetSOI=0xFFFFFFFF;

    pBS->DataFlag = MFX_BITSTREAM_COMPLETE_FRAME;

    while ((offsetSOI = FindMarker(pBS,pBS->DataOffset,CJPEGFrameReader::SOI))==0xFFFFFFFF && sts == MFX_ERR_NONE)
    {
        sts = CSmplBitstreamReader::ReadNextFrame(pBS);
    }

    //--- Finding EOI of frame, to make sure that it is complete
    while (FindMarker(pBS,offsetSOI,CJPEGFrameReader::EOI)==0xFFFFFFFF && sts == MFX_ERR_NONE)
    {
        sts = CSmplBitstreamReader::ReadNextFrame(pBS);
    }

    return sts;
}

CIVFFrameReader::CIVFFrameReader()
{
    MSDK_ZERO_MEMORY(m_hdr);
}

#define READ_BYTES(pBuf, size)\
{\
    mfxU32 nBytesRead = (mfxU32)fread(pBuf, 1, size, m_fSource);\
    if (nBytesRead !=size)\
        return MFX_ERR_MORE_DATA;\
}\

mfxStatus CIVFFrameReader::ReadHeader()
{
    // read and skip IVF header
    READ_BYTES(&m_hdr.dkif, sizeof(m_hdr.dkif));
    READ_BYTES(&m_hdr.version, sizeof(m_hdr.version));
    READ_BYTES(&m_hdr.header_len, sizeof(m_hdr.header_len));
    READ_BYTES(&m_hdr.codec_FourCC, sizeof(m_hdr.codec_FourCC));
    READ_BYTES(&m_hdr.width, sizeof(m_hdr.width));
    READ_BYTES(&m_hdr.height, sizeof(m_hdr.height));
    READ_BYTES(&m_hdr.frame_rate, sizeof(m_hdr.frame_rate));
    READ_BYTES(&m_hdr.time_scale, sizeof(m_hdr.time_scale));
    READ_BYTES(&m_hdr.num_frames, sizeof(m_hdr.num_frames));
    READ_BYTES(&m_hdr.unused, sizeof(m_hdr.unused));
    MSDK_CHECK_NOT_EQUAL(fseek(m_fSource, m_hdr.header_len, SEEK_SET), 0, MFX_ERR_UNSUPPORTED);
    return MFX_ERR_NONE;
}

void CIVFFrameReader::Reset()
{
    CSmplBitstreamReader::Reset();
    std::ignore = ReadHeader();
}

mfxStatus CIVFFrameReader::Init(const msdk_char *strFileName)
{
    mfxStatus sts = CSmplBitstreamReader::Init(strFileName);
    MSDK_CHECK_STATUS(sts, "CSmplBitstreamReader::Init failed");

    sts = ReadHeader();
    MSDK_CHECK_STATUS(sts, "CIVFFrameReader::ReadHeader failed");

    // check header
    MSDK_CHECK_NOT_EQUAL(MFX_MAKEFOURCC('D','K','I','F'), m_hdr.dkif, MFX_ERR_UNSUPPORTED);
    if ((m_hdr.codec_FourCC != MFX_MAKEFOURCC('V','P','8','0')) &&
        (m_hdr.codec_FourCC != MFX_MAKEFOURCC('V','P','9','0')) &&
        (m_hdr.codec_FourCC != MFX_MAKEFOURCC('A','V','0','1')))
    {
        return MFX_ERR_UNSUPPORTED;
    }

    return MFX_ERR_NONE;
}

// reads a complete frame into given bitstream
mfxStatus CIVFFrameReader::ReadNextFrame(mfxBitstream *pBS)
{
    MSDK_CHECK_POINTER(pBS, MFX_ERR_NULL_PTR);

    memmove(pBS->Data, pBS->Data + pBS->DataOffset, pBS->DataLength);
    pBS->DataOffset = 0;
    pBS->DataFlag = MFX_BITSTREAM_COMPLETE_FRAME;

    /*bytes pos-(pos+3)                       size of frame in bytes (not including the 12-byte header)
      bytes (pos+4)-(pos+11)                  64-bit presentation timestamp
      bytes (pos+12)-(pos+12+nBytesInFrame)   frame data
    */

    mfxU32 nBytesInFrame = 0;
    mfxU64 nTimeStamp = 0;

    // read frame size
    READ_BYTES(&nBytesInFrame, sizeof(nBytesInFrame));
    CHECK_SET_EOS(pBS);

    // read time stamp
    READ_BYTES(&nTimeStamp, sizeof(nTimeStamp));
    CHECK_SET_EOS(pBS);

    //check if bitstream has enough space to hold the frame
    if (nBytesInFrame > pBS->MaxLength - pBS->DataLength - pBS->DataOffset)
        return MFX_ERR_NOT_ENOUGH_BUFFER;

    // read frame data
    READ_BYTES(pBS->Data + pBS->DataOffset + pBS->DataLength, nBytesInFrame);
    CHECK_SET_EOS(pBS);
    pBS->DataLength += nBytesInFrame;

    // it is application's responsibility to make sure the bitstream contains a single complete frame and nothing else
    // application has to provide input pBS with pBS->DataLength = 0

    return MFX_ERR_NONE;
}


CSmplYUVWriter::CSmplYUVWriter()
{
    m_bInited = false;
    m_bIsMultiView = false;
    m_fDest = NULL;
    m_fDestMVC = NULL;
    m_numCreatedFiles = 0;
    m_nViews = 0;
};

mfxStatus CSmplYUVWriter::Init(const msdk_char *strFileName, const mfxU32 numViews)
{
    MSDK_CHECK_POINTER(strFileName, MFX_ERR_NULL_PTR);
    MSDK_CHECK_ERROR(msdk_strlen(strFileName), 0, MFX_ERR_NOT_INITIALIZED);

    m_sFile = msdk_string(strFileName);
    m_nViews = numViews;

    Close();

    //open file to write decoded data

    if (!m_bIsMultiView)
    {
        MSDK_FOPEN(m_fDest, m_sFile.c_str(), MSDK_STRING("wb"));
        MSDK_CHECK_POINTER(m_fDest, MFX_ERR_NULL_PTR);
        ++m_numCreatedFiles;
    }
    else
    {
        mfxU32 i;

        MSDK_CHECK_ERROR(numViews, 0, MFX_ERR_NOT_INITIALIZED);

        m_fDestMVC = new FILE*[numViews];
        for (i = 0; i < numViews; ++i)
        {
            MSDK_FOPEN(m_fDestMVC[i], FormMVCFileName(m_sFile.c_str(), i).c_str(), MSDK_STRING("wb"));
            MSDK_CHECK_POINTER(m_fDestMVC[i], MFX_ERR_NULL_PTR);
            ++m_numCreatedFiles;
        }
    }

    m_bInited = true;

    return MFX_ERR_NONE;
}

mfxStatus CSmplYUVWriter::Reset()
{
    if (!m_bInited)
        return MFX_ERR_NONE;

    return Init(m_sFile.c_str(), m_nViews);
}

CSmplYUVWriter::~CSmplYUVWriter()
{
    Close();
}

void CSmplYUVWriter::Close()
{
    if (m_fDest)
    {
        fclose(m_fDest);
        m_fDest = NULL;
    }

    if (m_fDestMVC)
    {
        mfxU32 i = 0;
        for (i = 0; i < m_numCreatedFiles; ++i)
        {
            if  (m_fDestMVC[i] != NULL)
            {
                fclose(m_fDestMVC[i]);
                m_fDestMVC[i] = NULL;
            }
        }
        delete [] m_fDestMVC;
        m_fDestMVC = NULL;
    }

    m_numCreatedFiles = 0;
    m_bInited = false;
}

mfxStatus GetChromaSize(const mfxFrameInfo & pInfo, mfxU32 & ChromaW, mfxU32 & ChromaH)
{
    switch (pInfo.FourCC)
    {
    case MFX_FOURCC_YV12:
    {
        ChromaW = (pInfo.CropW + 1) / 2;
        ChromaH = (pInfo.CropH + 1) / 2;
        break;
    }
    case MFX_FOURCC_NV12:
    {
        ChromaW = (pInfo.CropW % 2) ? (pInfo.CropW + 1) : pInfo.CropW;
        ChromaH = (pInfo.CropH + 1) / 2;
        break;
    }
    case MFX_FOURCC_P010:
    case MFX_FOURCC_P016:
    {
        ChromaW = (pInfo.CropW % 2) ? (pInfo.CropW + 1) : pInfo.CropW;
        ChromaH = (mfxU32)(pInfo.CropH + 1) / 2;
        break;
    }

    case MFX_FOURCC_P210:
    case MFX_FOURCC_Y210:
    case MFX_FOURCC_Y216:
    {
        ChromaW = (pInfo.CropW % 2) ? (pInfo.CropW + 1) : pInfo.CropW;
        ChromaH = pInfo.CropH;
        break;
    }

    case MFX_FOURCC_RGB4:
    case MFX_FOURCC_AYUV:
    case MFX_FOURCC_YUY2:
    case MFX_FOURCC_NV16:
    case MFX_FOURCC_A2RGB10:
    case MFX_FOURCC_Y410:
    case MFX_FOURCC_Y416:
    {
        if (pInfo.CropH > 0 && pInfo.CropW > 0)
        {
            ChromaW = pInfo.FourCC == MFX_FOURCC_YUY2 ? (pInfo.CropW + 1) / 2 : pInfo.CropW;
            ChromaH = pInfo.CropH;
        }
        else
        {
            ChromaW = pInfo.FourCC == MFX_FOURCC_YUY2 ? (pInfo.Width + 1) / 2 : pInfo.Width;
            ChromaH = pInfo.Height;
        }
        break;
    }

    default:
        return MFX_ERR_UNSUPPORTED;
    }

    return MFX_ERR_NONE;
}

mfxStatus CSmplYUVWriter::WriteNextFrame(mfxFrameSurface1 *pSurface)
{
    MSDK_CHECK_ERROR(m_bInited, false, MFX_ERR_NOT_INITIALIZED);
    MSDK_CHECK_POINTER(pSurface, MFX_ERR_NULL_PTR);

    mfxFrameInfo &pInfo = pSurface->Info;
    mfxFrameData &pData = pSurface->Data;

    mfxU32 i;
    mfxU32 vid = pInfo.FrameId.ViewId;

    mfxU32 shiftSizeLuma   = 16 - pInfo.BitDepthLuma;
    mfxU32 shiftSizeChroma = 16 - pInfo.BitDepthChroma;
    // Temporary buffer to convert MS to no-MS format
    std::vector<mfxU16> tmp;

    if (!m_bIsMultiView)
    {
        MSDK_CHECK_POINTER(m_fDest, MFX_ERR_NULL_PTR);
    }
    else
    {
        MSDK_CHECK_POINTER(m_fDestMVC, MFX_ERR_NULL_PTR);
        MSDK_CHECK_POINTER(m_fDestMVC[vid], MFX_ERR_NULL_PTR);
    }

    FILE* dstFile = m_bIsMultiView ? m_fDestMVC[vid] : m_fDest;

    mfxU32 ChromaW, ChromaH;
    if (MFX_ERR_NONE != GetChromaSize(pInfo, ChromaW, ChromaH))
        return MFX_ERR_UNSUPPORTED;

    switch (pInfo.FourCC)
    {
    case MFX_FOURCC_YV12:
    case MFX_FOURCC_NV12:
    case MFX_FOURCC_NV16:
        for (i = 0; i < pInfo.CropH; i++)
        {
            MSDK_CHECK_NOT_EQUAL(
                fwrite(pData.Y + (pInfo.CropY * pData.Pitch + pInfo.CropX) + i * pData.Pitch, 1, pInfo.CropW, dstFile),
                pInfo.CropW, MFX_ERR_UNDEFINED_BEHAVIOR);
        }
        break;
#if (MFX_VERSION >= 1027)
    case MFX_FOURCC_Y210:
#if (MFX_VERSION >= 1031)
    case MFX_FOURCC_Y216: // Luma and chroma will be filled below
#endif
    {
        for (i = 0; i < pInfo.CropH; i++)
        {
            mfxU8* pBuffer = ((mfxU8*)pData.Y) + (pInfo.CropY * pData.Pitch + pInfo.CropX * 4) + i * pData.Pitch;
            if (pInfo.Shift)
            {
                // Bits will be shifted to the lower position
                tmp.resize(pInfo.CropW * 2);

                for (int idx = 0; idx < pInfo.CropW*2; idx++)
                {
                    tmp[idx] = ((mfxU16*)pBuffer)[idx] >> shiftSizeLuma;
                }

                MSDK_CHECK_NOT_EQUAL(
                    fwrite(((const mfxU8*)tmp.data()), 4, pInfo.CropW, dstFile),
                    pInfo.CropW, MFX_ERR_UNDEFINED_BEHAVIOR);
            }
            else
            {
                MSDK_CHECK_NOT_EQUAL(
                    fwrite(pBuffer, 4, pInfo.CropW, dstFile),
                    pInfo.CropW, MFX_ERR_UNDEFINED_BEHAVIOR);
            }
        }
        return MFX_ERR_NONE;
    }
    break;
#endif
#if (MFX_VERSION >= 1027)
    case MFX_FOURCC_Y410: // Luma and chroma will be filled below
    {
        mfxU8* pBuffer = (mfxU8*)pData.Y410;
        for (i = 0; i < pInfo.CropH; i++)
        {
            MSDK_CHECK_NOT_EQUAL(
                fwrite(pBuffer + (pInfo.CropY * pData.Pitch + pInfo.CropX * 4) + i * pData.Pitch, 4, pInfo.CropW, dstFile),
                pInfo.CropW, MFX_ERR_UNDEFINED_BEHAVIOR);
        }
        return MFX_ERR_NONE;
    }
    break;
#endif
#if (MFX_VERSION >= 1031)
    case MFX_FOURCC_Y416:  // Luma and chroma will be filled below
    {
        for (i = 0; i < pInfo.CropH; i++)
        {
            mfxU8* pBuffer = ((mfxU8*)pData.U) + (pInfo.CropY * pData.Pitch + pInfo.CropX * 8) + i * pData.Pitch;
            if (pInfo.Shift)
            {
                tmp.resize(pInfo.CropW * 4);

                for (int idx = 0; idx < pInfo.CropW*4; idx++)
                {
                    tmp[idx] = ((mfxU16*)pBuffer)[idx] >> shiftSizeLuma;
                }

                MSDK_CHECK_NOT_EQUAL(
                    fwrite(((const mfxU8*)tmp.data()), 8, pInfo.CropW, dstFile),
                    pInfo.CropW, MFX_ERR_UNDEFINED_BEHAVIOR);
            }
            else
            {
                MSDK_CHECK_NOT_EQUAL(
                    fwrite(pBuffer, 8, pInfo.CropW, dstFile),
                    pInfo.CropW, MFX_ERR_UNDEFINED_BEHAVIOR);
            }
        }
        return MFX_ERR_NONE;
    }
    break;
#endif
    case MFX_FOURCC_P010:
#if (MFX_VERSION >= 1031)
    case MFX_FOURCC_P016:
#endif
    case MFX_FOURCC_P210:
    {
        for (i = 0; i < pInfo.CropH; i++)
        {
            mfxU16* shortPtr = (mfxU16*)(pData.Y + (pInfo.CropY * pData.Pitch + pInfo.CropX) + i * pData.Pitch);
            if (pInfo.Shift)
            {
                // Convert MS-P*1* to P*1* and write
                // Bits will be shifted to the lower position
                tmp.resize(pData.Pitch);

                for (int idx = 0; idx < pInfo.CropW; idx++)
                {
                    tmp[idx] = shortPtr[idx] >> shiftSizeLuma;
                }

                MSDK_CHECK_NOT_EQUAL(
                    fwrite(&tmp[0], 1, (mfxU32)pInfo.CropW * 2, dstFile),
                    (mfxU32)pInfo.CropW * 2, MFX_ERR_UNDEFINED_BEHAVIOR);

            }
            else
            {
                MSDK_CHECK_NOT_EQUAL(
                    fwrite(shortPtr, 1, (mfxU32)pInfo.CropW * 2, dstFile),
                    (mfxU32)pInfo.CropW * 2, MFX_ERR_UNDEFINED_BEHAVIOR);
            }
        }

        break;
    }
    case MFX_FOURCC_RGB4:
    case MFX_FOURCC_AYUV:
    case MFX_FOURCC_A2RGB10:
    case MFX_FOURCC_YUY2:
        // Implementation for these formats is in the next switch below
        break;

    default:
        return MFX_ERR_UNSUPPORTED;
    }
    switch (pInfo.FourCC)
    {
    case MFX_FOURCC_YV12:
    {
        for (i = 0; i < ChromaH; i++)
        {
            MSDK_CHECK_NOT_EQUAL(
                fwrite(pData.V + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX / 2) + i * pData.Pitch, 1, ChromaW, dstFile),
                ChromaW, MFX_ERR_UNDEFINED_BEHAVIOR);
        }
        for (i = 0; i < ChromaH; i++)
        {
            MSDK_CHECK_NOT_EQUAL(
                fwrite(pData.U + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX / 2) + i * pData.Pitch / 2, 1, ChromaW, dstFile),
                ChromaW, MFX_ERR_UNDEFINED_BEHAVIOR);
        }
        break;
    }
    case MFX_FOURCC_NV12:
    {
        for (i = 0; i < ChromaH; i++)
        {
            MSDK_CHECK_NOT_EQUAL(
                fwrite(pData.UV + (pInfo.CropY * pData.Pitch + pInfo.CropX) + i * pData.Pitch, 1, ChromaW, dstFile),
                ChromaW, MFX_ERR_UNDEFINED_BEHAVIOR);
        }
        break;
    }
    case MFX_FOURCC_NV16:
    {
        for (i = 0; i < ChromaH; i++)
        {
            MSDK_CHECK_NOT_EQUAL(
                fwrite(pData.UV + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX) + i * pData.Pitch, 1, ChromaW, dstFile),
                ChromaW, MFX_ERR_UNDEFINED_BEHAVIOR);
        }
        break;
    }
    case MFX_FOURCC_P010:
#if (MFX_VERSION >= 1031)
    case MFX_FOURCC_P016:
#endif
    case MFX_FOURCC_P210:
    {
        for (i = 0; i < ChromaH; i++)
        {
            mfxU16* shortPtr = (mfxU16*)(pData.UV + (pInfo.CropY * pData.Pitch + pInfo.CropX*2) + i * pData.Pitch);
            if (pInfo.Shift)
            {
                // Convert MS-P*1* to P*1* and write
                // Bits will be shifted to the lower position
                tmp.resize(pData.Pitch);

                for (mfxU32 idx = 0; idx < ChromaW; idx++)
                {
                    tmp[idx] = shortPtr[idx] >> shiftSizeChroma;
                }

                MSDK_CHECK_NOT_EQUAL(
                    fwrite(&tmp[0], 1, ChromaW * 2, dstFile),
                    (mfxU32)ChromaW * 2, MFX_ERR_UNDEFINED_BEHAVIOR);

            }
            else
            {
                MSDK_CHECK_NOT_EQUAL(
                    fwrite(shortPtr, 1, ChromaW * 2, dstFile),
                    ChromaW * 2, MFX_ERR_UNDEFINED_BEHAVIOR);
            }
        }
        break;
    }

    case MFX_FOURCC_RGB4:
    case MFX_FOURCC_AYUV:
    case MFX_FOURCC_YUY2:
    case MFX_FOURCC_A2RGB10:
    {
        mfxU8* ptr;

        ptr = std::min({pData.R, pData.G, pData.B});
        ptr = ptr + pInfo.CropX + pInfo.CropY * pData.Pitch;

        for (i = 0; i < ChromaH; i++)
        {
            MSDK_CHECK_NOT_EQUAL(fwrite(ptr + i * pData.Pitch, 1, 4 * ChromaW, dstFile), 4 * ChromaW, MFX_ERR_UNDEFINED_BEHAVIOR);
        }
        fflush(dstFile);
        break;
    }

    default:
        return MFX_ERR_UNSUPPORTED;
    }

    return MFX_ERR_NONE;
}

mfxStatus CSmplYUVWriter::WriteNextFrameI420(mfxFrameSurface1 *pSurface)
{
    MSDK_CHECK_ERROR(m_bInited, false,   MFX_ERR_NOT_INITIALIZED);
    MSDK_CHECK_POINTER(pSurface,         MFX_ERR_NULL_PTR);

    mfxFrameInfo &pInfo = pSurface->Info;
    mfxFrameData &pData = pSurface->Data;

    mfxU32 i, j;
    mfxU32 vid = pInfo.FrameId.ViewId;

    if (!m_bIsMultiView)
    {
        MSDK_CHECK_POINTER(m_fDest, MFX_ERR_NULL_PTR);
    }
    else
    {
        MSDK_CHECK_POINTER(m_fDestMVC, MFX_ERR_NULL_PTR);
        MSDK_CHECK_POINTER(m_fDestMVC[vid], MFX_ERR_NULL_PTR);
    }

    mfxU32 ChromaW, ChromaH;
    if (MFX_ERR_NONE != GetChromaSize(pInfo, ChromaW, ChromaH))
        return MFX_ERR_UNSUPPORTED;

    // Write Y
    switch (pInfo.FourCC)
    {
        case MFX_FOURCC_YV12:
        case MFX_FOURCC_NV12:
        {
            for (i = 0; i < pInfo.CropH; i++)
            {
                if (!m_bIsMultiView)
                {
                    MSDK_CHECK_NOT_EQUAL(
                        fwrite(pData.Y + (pInfo.CropY * pData.Pitch + pInfo.CropX)+ i * pData.Pitch, 1, pInfo.CropW, m_fDest),
                        pInfo.CropW, MFX_ERR_UNDEFINED_BEHAVIOR);
                }
                else
                {
                    MSDK_CHECK_NOT_EQUAL(
                        fwrite(pData.Y + (pInfo.CropY * pData.Pitch + pInfo.CropX)+ i * pData.Pitch, 1, pInfo.CropW, m_fDestMVC[vid]),
                        pInfo.CropW, MFX_ERR_UNDEFINED_BEHAVIOR);
                }
            }
            break;
        }
        default:
        {
            msdk_printf(MSDK_STRING("ERROR: I420 output is accessible only for NV12 and YV12.\n"));
            return MFX_ERR_UNSUPPORTED;
        }
    }

    // Write U and V
    switch (pInfo.FourCC)
    {
        case MFX_FOURCC_YV12:
        {
            for (i = 0; i < ChromaH; i++)
            {
                if (!m_bIsMultiView)
                {
                    MSDK_CHECK_NOT_EQUAL(
                        fwrite(pData.U + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX / 2)+ i * pData.Pitch / 2, 1, ChromaW, m_fDest),
                        (mfxU32)pInfo.CropW/2, MFX_ERR_UNDEFINED_BEHAVIOR);
                }
                else
                {
                    MSDK_CHECK_NOT_EQUAL(
                        fwrite(pData.U + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX / 2)+ i * pData.Pitch / 2, 1, ChromaW, m_fDestMVC[vid]),
                        (mfxU32)pInfo.CropW/2, MFX_ERR_UNDEFINED_BEHAVIOR);
                }
            }
            for (i = 0; i < ChromaH; i++)
            {
                if (!m_bIsMultiView)
                {
                    MSDK_CHECK_NOT_EQUAL(
                        fwrite(pData.V + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX / 2)+ i * pData.Pitch / 2, 1, ChromaW, m_fDest),
                        (mfxU32)pInfo.CropW/2, MFX_ERR_UNDEFINED_BEHAVIOR);
                }
                else
                {
                    MSDK_CHECK_NOT_EQUAL(
                        fwrite(pData.V + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX / 2)+ i * pData.Pitch / 2, 1, ChromaW, m_fDestMVC[vid]),
                        (mfxU32)pInfo.CropW/2, MFX_ERR_UNDEFINED_BEHAVIOR);
                }
            }
            break;
        }
        case MFX_FOURCC_NV12:
        {
            for (i = 0; i < ChromaH; i++)
            {
                for (j = 0; j < ChromaW; j += 2)
                {
                    if (!m_bIsMultiView)
                    {
                        MSDK_CHECK_NOT_EQUAL(
                            fwrite(pData.UV + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX) + i * pData.Pitch + j, 1, 1, m_fDest),
                            1, MFX_ERR_UNDEFINED_BEHAVIOR);
                    }
                    else
                    {
                        MSDK_CHECK_NOT_EQUAL(
                            fwrite(pData.UV + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX) + i * pData.Pitch + j, 1, 1, m_fDestMVC[vid]),
                            1, MFX_ERR_UNDEFINED_BEHAVIOR);
                    }
                }
            }
            for (i = 0; i < ChromaH; i++)
            {
                for (j = 1; j < ChromaW; j += 2)
                {
                    if (!m_bIsMultiView)
                    {
                        MSDK_CHECK_NOT_EQUAL(
                            fwrite(pData.UV + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX)+ i * pData.Pitch + j, 1, 1, m_fDest),
                            1, MFX_ERR_UNDEFINED_BEHAVIOR);
                    }
                    else
                    {
                        MSDK_CHECK_NOT_EQUAL(
                            fwrite(pData.UV + (pInfo.CropY * pData.Pitch / 2 + pInfo.CropX)+ i * pData.Pitch + j, 1, 1, m_fDestMVC[vid]),
                            1, MFX_ERR_UNDEFINED_BEHAVIOR);
                    }
                }
            }
            break;
        }
        default:
        {
            msdk_printf(MSDK_STRING("ERROR: I420 output is accessible only for NV12 and YV12.\n"));
            return MFX_ERR_UNSUPPORTED;
        }
    }

    return MFX_ERR_NONE;
}

void QPFile::Reader::ResetState()
{
    ResetState(READER_ERR_NOT_INITIALIZED);
}

void QPFile::Reader::ResetState(ReaderStatus set_sts)
{
    m_CurFrameNum = std::numeric_limits<mfxU32>::max();
    m_nFrames     = std::numeric_limits<mfxU32>::max();
    m_ReaderSts   = set_sts;
    m_FrameVals.clear();
}

mfxStatus QPFile::Reader::Read(const msdk_string& strFileName, mfxU32 codecid)
{
    m_ReaderSts   = READER_ERR_NONE;
    m_CurFrameNum = 0;

    if (codecid != MFX_CODEC_AVC && codecid != MFX_CODEC_HEVC)
    {
        ResetState(READER_ERR_CODEC_UNSUPPORTED);
        return MFX_ERR_NOT_INITIALIZED;
    }

    std::ifstream ifs(strFileName, msdk_fstream::in);
    if (!ifs.is_open())
    {
        ResetState(READER_ERR_FILE_NOT_OPEN);
        return MFX_ERR_NOT_INITIALIZED;
    }

    FrameInfo   frameInfo{};
    std::string line;
    std::getline(ifs, line);
    m_nFrames = std::stoi(line); // number of frames at first line

    m_FrameVals.reserve(m_nFrames);

    while (QPFile::get_line(ifs, line))
    {
        frameInfo.displayOrder = QPFile::ReadDisplayOrder(line);
        frameInfo.QP = QPFile::ReadQP(line);
        frameInfo.frameType = QPFile::ReadFrameType(line);
        if ( frameInfo.displayOrder > m_nFrames ||
                frameInfo.QP > 51 ||
                frameInfo.frameType == MFX_FRAMETYPE_UNKNOWN )
        {
            ResetState(READER_ERR_INCORRECT_FILE);
            return MFX_ERR_NOT_INITIALIZED;
        }
        m_FrameVals.push_back(frameInfo);
    }
    if (m_FrameVals.size() < m_nFrames)
    {
        ResetState(READER_ERR_INCORRECT_FILE);
        return MFX_ERR_NOT_INITIALIZED;
    }

    return MFX_ERR_NONE;
}

std::string QPFile::Reader::GetErrorMessage() const   { return ReaderStatusToString(m_ReaderSts); }
mfxU32 QPFile::Reader::GetCurrentEncodedOrder() const { return m_CurFrameNum; }
mfxU32 QPFile::Reader::GetCurrentDisplayOrder() const { return m_FrameVals.at(m_CurFrameNum).displayOrder; }
mfxU16 QPFile::Reader::GetCurrentQP() const           { return m_FrameVals.at(m_CurFrameNum).QP; }
mfxU16 QPFile::Reader::GetCurrentFrameType() const    { return m_FrameVals.at(m_CurFrameNum).frameType; }
mfxU32 QPFile::Reader::GetFramesNum() const           { return m_nFrames; }
void   QPFile::Reader::NextFrame()                    { ++m_CurFrameNum; }


void TCBRCTestFile::Reader::ResetState(ReaderStatus set_sts)
{
    m_CurFrameNum = std::numeric_limits<mfxU32>::max();
    m_ReaderSts = set_sts;
    m_FrameVals.clear();
}

mfxStatus TCBRCTestFile::Reader::Read(const msdk_string& strFileName, mfxU32 codecid)
{
    m_ReaderSts = READER_ERR_NONE;
    m_CurFrameNum = 0;

    if (codecid != MFX_CODEC_AVC && codecid != MFX_CODEC_HEVC)
    {
        ResetState(READER_ERR_CODEC_UNSUPPORTED);
        return MFX_ERR_NOT_INITIALIZED;
    }

    std::ifstream ifs(strFileName, msdk_fstream::in);
    if (!ifs.is_open())
    {
        ResetState(READER_ERR_FILE_NOT_OPEN);
        return MFX_ERR_NOT_INITIALIZED;
    }

    FrameInfo   frameInfo{};
    std::string line;


    mfxU32 n = 0;
    while (TCBRCTestFile::get_line(ifs, line))
    {
        frameInfo.displayOrder = TCBRCTestFile::ReadDisplayOrder(line);
        frameInfo.targetFrameSize = TCBRCTestFile::ReadTargetFrameSize(line);
        if (frameInfo.displayOrder==0)
            frameInfo.displayOrder  = n;
        m_FrameVals.push_back(frameInfo);
        n++;
    }
    return MFX_ERR_NONE;
}
std::string TCBRCTestFile::Reader::GetErrorMessage() const { return ReaderStatusToString(m_ReaderSts); }
mfxU32 TCBRCTestFile::Reader::GetTargetFrameSize(mfxU32 displayOrder) const
{ 
    mfxU32 num = (mfxU32)m_FrameVals.size();
    if (num == 0) return 0;
    for (mfxU32 i = 0; i < num - 1; i++)
    {
        if (m_FrameVals.at(i + 1).displayOrder > displayOrder)
            return m_FrameVals.at(i).targetFrameSize;

    }
    return m_FrameVals.at(num - 1).targetFrameSize;
}

mfxStatus ConvertFrameRate(mfxF64 dFrameRate, mfxU32* pnFrameRateExtN, mfxU32* pnFrameRateExtD)
{
    MSDK_CHECK_POINTER(pnFrameRateExtN, MFX_ERR_NULL_PTR);
    MSDK_CHECK_POINTER(pnFrameRateExtD, MFX_ERR_NULL_PTR);

    mfxU32 fr;

    fr = (mfxU32)(dFrameRate + .5);

    if (fabs(fr - dFrameRate) < 0.0001)
    {
        *pnFrameRateExtN = fr;
        *pnFrameRateExtD = 1;
        return MFX_ERR_NONE;
    }

    fr = (mfxU32)(dFrameRate * 1.001 + .5);

    if (fabs(fr * 1000 - dFrameRate * 1001) < 10)
    {
        *pnFrameRateExtN = fr * 1000;
        *pnFrameRateExtD = 1001;
        return MFX_ERR_NONE;
    }

    *pnFrameRateExtN = (mfxU32)(dFrameRate * 10000 + .5);
    *pnFrameRateExtD = 10000;

    return MFX_ERR_NONE;
}

mfxF64 CalculateFrameRate(mfxU32 nFrameRateExtN, mfxU32 nFrameRateExtD)
{
    if (nFrameRateExtN && nFrameRateExtD)
        return (mfxF64)nFrameRateExtN / nFrameRateExtD;
    else
        return 0;
}

void FreeSurfacePool(mfxFrameSurface1* pSurfacesPool, mfxU16 nPoolSize)
{
    if (pSurfacesPool)
    {
        for (mfxU16 i = 0; i < nPoolSize; i++)
        {
            pSurfacesPool[i].Data.Locked = 0;
        }
    }
}

mfxU16 GetFreeSurface(mfxFrameSurface1* pSurfacesPool, mfxU16 nPoolSize)
{
    mfxU32 SleepInterval = 10; // milliseconds

    mfxU16 idx = MSDK_INVALID_SURF_IDX;

    CTimer t;
    t.Start();
    //wait if there's no free surface
    do
    {
        idx = GetFreeSurfaceIndex(pSurfacesPool, nPoolSize);

        if (MSDK_INVALID_SURF_IDX != idx)
        {
            break;
        }
        else
        {
            MSDK_SLEEP(SleepInterval);
        }
    } while ( t.GetTime() < MSDK_SURFACE_WAIT_INTERVAL / 1000 );

    if(idx==MSDK_INVALID_SURF_IDX)
    {
        msdk_printf(MSDK_STRING("ERROR: No free surfaces in pool (during long period)\n"));
    }

    return idx;
}

std::basic_string<msdk_char> CodecIdToStr(mfxU32 nFourCC)
{
    std::basic_string<msdk_char> fcc;
    for (size_t i = 0; i < 4; i++)
    {
        fcc.push_back((msdk_char)*(i + (char*)&nFourCC));
    }
    return fcc;
}

PartiallyLinearFNC::PartiallyLinearFNC()
: m_pX()
, m_pY()
, m_nPoints()
, m_nAllocated()
{
}

PartiallyLinearFNC::~PartiallyLinearFNC()
{
    delete []m_pX;
    m_pX = NULL;
    delete []m_pY;
    m_pY = NULL;
}

void PartiallyLinearFNC::AddPair(mfxF64 x, mfxF64 y)
{
    //duplicates searching
    for (mfxU32 i = 0; i < m_nPoints; i++)
    {
        if (m_pX[i] == x)
            return;
    }
    if (m_nPoints == m_nAllocated)
    {
        m_nAllocated += 20;
        mfxF64 * pnew;
        pnew = new mfxF64[m_nAllocated];
        //memcpy_s(pnew, sizeof(mfxF64)*m_nAllocated, m_pX, sizeof(mfxF64) * m_nPoints);
        MSDK_MEMCPY_BUF(pnew,0,sizeof(mfxF64)*m_nAllocated, m_pX,sizeof(mfxF64) * m_nPoints);
        delete [] m_pX;
        m_pX = pnew;

        pnew = new mfxF64[m_nAllocated];
        //memcpy_s(pnew, sizeof(mfxF64)*m_nAllocated, m_pY, sizeof(mfxF64) * m_nPoints);
        MSDK_MEMCPY_BUF(pnew,0,sizeof(mfxF64)*m_nAllocated, m_pY,sizeof(mfxF64) * m_nPoints);
        delete [] m_pY;
        m_pY = pnew;
    }
    m_pX[m_nPoints] = x;
    m_pY[m_nPoints] = y;

    m_nPoints ++;
}

mfxF64 PartiallyLinearFNC::at(mfxF64 x)
{
    if (m_nPoints < 2)
    {
        return 0;
    }
    bool bwasmin = false;
    bool bwasmax = false;

    mfxU32 maxx = 0;
    mfxU32 minx = 0;
    mfxU32 i;

    for (i=0; i < m_nPoints; i++)
    {
        if (m_pX[i] <= x && (!bwasmin || m_pX[i] > m_pX[maxx]))
        {
            maxx = i;
            bwasmin = true;
        }
        if (m_pX[i] > x && (!bwasmax || m_pX[i] < m_pX[minx]))
        {
            minx = i;
            bwasmax = true;
        }
    }

    //point on the left
    if (!bwasmin)
    {
        for (i=0; i < m_nPoints; i++)
        {
            if (m_pX[i] > m_pX[minx] && (!bwasmin || m_pX[i] < m_pX[minx]))
            {
                maxx = i;
                bwasmin = true;
            }
        }
    }
    //point on the right
    if (!bwasmax)
    {
        for (i=0; i < m_nPoints; i++)
        {
            if (m_pX[i] < m_pX[maxx] && (!bwasmax || m_pX[i] > m_pX[minx]))
            {
                minx = i;
                bwasmax = true;
            }
        }
    }

    //linear interpolation
    return (x - m_pX[minx])*(m_pY[maxx] - m_pY[minx]) / (m_pX[maxx] - m_pX[minx]) + m_pY[minx];
}

mfxU16 CalculateDefaultBitrate(mfxU32 nCodecId, mfxU32 nTargetUsage, mfxU32 nWidth, mfxU32 nHeight, mfxF64 dFrameRate)
{
    PartiallyLinearFNC fnc;
    mfxF64 bitrate = 0;

    switch (nCodecId)
    {
    case MFX_CODEC_HEVC :
    {
        fnc.AddPair(0, 0);
        fnc.AddPair(25344, 225/1.3);
        fnc.AddPair(101376, 1000/1.3);
        fnc.AddPair(414720, 4000/1.3);
        fnc.AddPair(2058240, 5000/1.3);
        break;
    }
    case MFX_CODEC_AVC :
        {
            fnc.AddPair(0, 0);
            fnc.AddPair(25344, 225);
            fnc.AddPair(101376, 1000);
            fnc.AddPair(414720, 4000);
            fnc.AddPair(2058240, 5000);
            break;
        }
    case MFX_CODEC_MPEG2:
        {
            fnc.AddPair(0, 0);
            fnc.AddPair(414720, 12000);
            break;
        }
    default:
        {
            fnc.AddPair(0, 0);
            fnc.AddPair(414720, 12000);
            break;
        }
    }

    mfxF64 at = nWidth * nHeight * dFrameRate / 30.0;

    if (!at)
        return 0;

    switch (nTargetUsage)
    {
    case MFX_TARGETUSAGE_BEST_QUALITY :
        {
            bitrate = (&fnc)->at(at);
            break;
        }
    case MFX_TARGETUSAGE_BEST_SPEED :
        {
            bitrate = (&fnc)->at(at) * 0.5;
            break;
        }
    case MFX_TARGETUSAGE_BALANCED :
    default:
        {
            bitrate = (&fnc)->at(at) * 0.75;
            break;
        }
    }

    return (mfxU16)bitrate;
}

mfxU16 StrToTargetUsage(msdk_string strInput)
{
    std::map<msdk_string, mfxU16> tu;
    tu[MSDK_STRING("quality")] =  (mfxU16)MFX_TARGETUSAGE_1;
    tu[MSDK_STRING("veryslow")] = (mfxU16)MFX_TARGETUSAGE_1;
    tu[MSDK_STRING("slower")] =   (mfxU16)MFX_TARGETUSAGE_2;
    tu[MSDK_STRING("slow")] =     (mfxU16)MFX_TARGETUSAGE_3;
    tu[MSDK_STRING("medium")] =   (mfxU16)MFX_TARGETUSAGE_4;
    tu[MSDK_STRING("balanced")] = (mfxU16)MFX_TARGETUSAGE_4;
    tu[MSDK_STRING("fast")] =     (mfxU16)MFX_TARGETUSAGE_5;
    tu[MSDK_STRING("faster")] =   (mfxU16)MFX_TARGETUSAGE_6;
    tu[MSDK_STRING("veryfast")] = (mfxU16)MFX_TARGETUSAGE_7;
    tu[MSDK_STRING("speed")] =    (mfxU16)MFX_TARGETUSAGE_7;
    tu[MSDK_STRING("1")] =        (mfxU16)MFX_TARGETUSAGE_1;
    tu[MSDK_STRING("2")] =        (mfxU16)MFX_TARGETUSAGE_2;
    tu[MSDK_STRING("3")] =        (mfxU16)MFX_TARGETUSAGE_3;
    tu[MSDK_STRING("4")] =        (mfxU16)MFX_TARGETUSAGE_4;
    tu[MSDK_STRING("5")] =        (mfxU16)MFX_TARGETUSAGE_5;
    tu[MSDK_STRING("6")] =        (mfxU16)MFX_TARGETUSAGE_6;
    tu[MSDK_STRING("7")] =        (mfxU16)MFX_TARGETUSAGE_7;

    if (tu.find(strInput) == tu.end())
        return 0;
    else
        return tu[strInput];
}

const msdk_char* TargetUsageToStr(mfxU16 tu)
{
    switch(tu)
    {
    case MFX_TARGETUSAGE_BALANCED:
        return MSDK_STRING("balanced");
    case MFX_TARGETUSAGE_BEST_QUALITY:
        return MSDK_STRING("quality");
    case MFX_TARGETUSAGE_BEST_SPEED:
        return MSDK_STRING("speed");
    case MFX_TARGETUSAGE_UNKNOWN:
        return MSDK_STRING("unknown");
    default:
        return MSDK_STRING("unsupported");
    }
}

const msdk_char* ColorFormatToStr(mfxU32 format)
{
    switch(format)
    {
    case MFX_FOURCC_NV12:
        return MSDK_STRING("NV12");
    case MFX_FOURCC_YV12:
        return MSDK_STRING("YV12");
    case MFX_FOURCC_I420:
        return MSDK_STRING("YUV420");
    case MFX_FOURCC_RGB4:
        return MSDK_STRING("RGB4");
    case MFX_FOURCC_YUY2:
        return MSDK_STRING("YUY2");
    case MFX_FOURCC_UYVY:
       return MSDK_STRING("UYVY");
    case MFX_FOURCC_P010:
       return MSDK_STRING("P010");
    case MFX_FOURCC_P210:
        return MSDK_STRING("P210");
#if (MFX_VERSION >= 1027)
    case MFX_FOURCC_Y210:
        return MSDK_STRING("Y210");
    case MFX_FOURCC_Y410:
        return MSDK_STRING("Y410");
#endif
#if (MFX_VERSION >= 1031)
    case MFX_FOURCC_P016:
        return MSDK_STRING("P016");
    case MFX_FOURCC_Y216:
        return MSDK_STRING("Y216");
#endif
    default:
        return MSDK_STRING("unsupported");
    }
}

mfxU32 GCD(mfxU32 a, mfxU32 b)
{
    if (0 == a)
        return b;
    else if (0 == b)
        return a;

    mfxU32 a1, b1;

    if (a >= b)
    {
        a1 = a;
        b1 = b;
    }
    else
    {
        a1 = b;
        b1 = a;
    }

    // a1 >= b1;
    mfxU32 r = a1 % b1;

    while (0 != r)
    {
        a1 = b1;
        b1 = r;
        r = a1 % b1;
    }

    return b1;
}



std::basic_string<msdk_char> FormMVCFileName(const msdk_char *strFileNamePattern, const mfxU32 numView)
{
    if (NULL == strFileNamePattern)
        return MSDK_STRING("");

    std::basic_string<msdk_char> fileName, mvcFileName, fileExt;
    fileName = strFileNamePattern;

    msdk_char postfixBuffer[4];
    msdk_itoa_decimal(numView, postfixBuffer);
    mvcFileName = fileName;
    mvcFileName.append(MSDK_STRING("_"));
    mvcFileName.append(postfixBuffer);
    mvcFileName.append(MSDK_STRING(".yuv"));

    return mvcFileName;
}

// function for getting a pointer to a specific external buffer from the array
mfxExtBuffer* GetExtBuffer(mfxExtBuffer** ebuffers, mfxU32 nbuffers, mfxU32 BufferId)
{
    if (!ebuffers) return 0;
    for(mfxU32 i=0; i<nbuffers; i++) {
        if (!ebuffers[i]) continue;
        if (ebuffers[i]->BufferId == BufferId) {
            return ebuffers[i];
        }
    }
    return 0;
}

mfxStatus MJPEG_AVI_ParsePicStruct(mfxBitstream *bitstream)
{
    // check input for consistency
    MSDK_CHECK_POINTER(bitstream->Data, MFX_ERR_MORE_DATA);
    if (bitstream->DataLength <= 0)
        return MFX_ERR_MORE_DATA;

    // define JPEG markers
    const mfxU8 APP0_marker [] = { 0xFF, 0xE0 };
    const mfxU8 SOI_marker  [] = { 0xFF, 0xD8 };
    const mfxU8 AVI1        [] = { 'A', 'V', 'I', '1' };

    // size of length field in header
    const mfxU8 len_size  = 2;
    // size of picstruct field in header
    const mfxU8 picstruct_size = 1;

    mfxU32 length = bitstream->DataLength;
    const mfxU8 *ptr = reinterpret_cast<const mfxU8*>(bitstream->Data);

    //search for SOI marker
    while ((length >= sizeof(SOI_marker)) && memcmp(ptr, SOI_marker, sizeof(SOI_marker)))
    {
        skip(ptr, length, (mfxU32)1);
    }

    // skip SOI
    if (!skip(ptr, length, (mfxU32)sizeof(SOI_marker)) || length < sizeof(APP0_marker))
        return MFX_ERR_MORE_DATA;

    // if there is no APP0 marker return
    if (memcmp(ptr, APP0_marker, sizeof(APP0_marker)))
    {
        bitstream->PicStruct = MFX_PICSTRUCT_UNKNOWN;
        return MFX_ERR_NONE;
    }

    // skip APP0 & length value
    if (!skip(ptr, length, (mfxU32)sizeof(APP0_marker) + len_size) || length < sizeof(AVI1))
        return MFX_ERR_MORE_DATA;

    if (memcmp(ptr, AVI1, sizeof(AVI1)))
    {
        bitstream->PicStruct = MFX_PICSTRUCT_UNKNOWN;
        return MFX_ERR_NONE;
    }

    // skip 'AVI1'
    if (!skip(ptr, length, (mfxU32)sizeof(AVI1)) || length < picstruct_size)
        return MFX_ERR_MORE_DATA;

    // get PicStruct
    switch (*ptr)
    {
        case 0:
            bitstream->PicStruct = MFX_PICSTRUCT_PROGRESSIVE;
            break;
        case 1:
            bitstream->PicStruct = MFX_PICSTRUCT_FIELD_TFF;
            break;
        case 2:
            bitstream->PicStruct = MFX_PICSTRUCT_FIELD_BFF;
            break;
        default:
            bitstream->PicStruct = MFX_PICSTRUCT_UNKNOWN;
    }

    return MFX_ERR_NONE;
}

mfxVersion getMinimalRequiredVersion(const APIChangeFeatures &features)
{
    mfxVersion version = {{1, 1}};

    if (features.MVCDecode || features.MVCEncode || features.LowLatency || features.JpegDecode)
    {
        version.Minor = 3;
    }

    if (features.ViewOutput)
    {
        version.Minor = 4;
    }

    if (features.JpegEncode || features.IntraRefresh)
    {
        version.Minor = 6;
    }

    if (features.LookAheadBRC)
    {
        version.Minor = 7;
    }

    if (features.AudioDecode) {
        version.Minor = 8;
    }

    if (features.SupportCodecPluginAPI) {
        version.Minor = 8;
    }

    return version;
}

bool CheckVersion(mfxVersion* version, msdkAPIFeature feature)
{
    if (!version) {
        return false;
    }

    mfxU32 ver = MakeVersion(version->Major, version->Minor);

    switch (feature) {
    case MSDK_FEATURE_NONE:
        return true;
    case MSDK_FEATURE_MVC:
        if (ver >= 1003) {
            return true;
        }
        break;
    case MSDK_FEATURE_JPEG_DECODE:
        if (ver >= 1003) {
            return true;
        }
        break;
   case MSDK_FEATURE_LOW_LATENCY:
        if (ver >= 1003) {
            return true;
        }
        break;
    case MSDK_FEATURE_MVC_VIEWOUTPUT:
        if (ver >= 1004) {
            return true;
        }
        break;
    case MSDK_FEATURE_JPEG_ENCODE:
        if (ver >= 1006) {
            return true;
        }
        break;
    case MSDK_FEATURE_LOOK_AHEAD:
        if (ver >= 1007) {
            return true;
        }
        break;
    case MSDK_FEATURE_PLUGIN_API:
        if (ver >= 1008) {
            return true;
        }
        break;
    default:
        return false;
    }
    return false;
}

void ConfigureAspectRatioConversion(mfxInfoVPP* pVppInfo)
{
    if (!pVppInfo) return;

    if (pVppInfo->In.AspectRatioW &&
        pVppInfo->In.AspectRatioH &&
        pVppInfo->In.CropW &&
        pVppInfo->In.CropH &&
        pVppInfo->Out.AspectRatioW &&
        pVppInfo->Out.AspectRatioH &&
        pVppInfo->Out.CropW &&
        pVppInfo->Out.CropH)
    {
        mfxF64 dFrameAR         = ((mfxF64)pVppInfo->In.AspectRatioW * pVppInfo->In.CropW) /
                                   (mfxF64)pVppInfo->In.AspectRatioH /
                                   (mfxF64)pVppInfo->In.CropH;

        mfxF64 dPixelAR         = pVppInfo->Out.AspectRatioW / (mfxF64)pVppInfo->Out.AspectRatioH;

        mfxU16 dProportionalH   = (mfxU16)(pVppInfo->Out.CropW * dPixelAR / dFrameAR + 1) & -2; //round to closest odd (values are always positive)

        if (dProportionalH < pVppInfo->Out.CropH)
        {
            pVppInfo->Out.CropY = (mfxU16)((pVppInfo->Out.CropH - dProportionalH) / 2. + 1) & -2;
            pVppInfo->Out.CropH = pVppInfo->Out.CropH - 2 * pVppInfo->Out.CropY;
        }
        else if (dProportionalH > pVppInfo->Out.CropH)
        {
            mfxU16 dProportionalW = (mfxU16)(pVppInfo->Out.CropH * dFrameAR / dPixelAR + 1) & -2;

            pVppInfo->Out.CropX = (mfxU16)((pVppInfo->Out.CropW - dProportionalW) / 2 + 1) & -2;
            pVppInfo->Out.CropW = pVppInfo->Out.CropW - 2 * pVppInfo->Out.CropX;
        }
    }
}

void SEICalcSizeType(std::vector<mfxU8>& data, mfxU16 type, mfxU32 size)
{
    mfxU32 B = type;

    while (B > 255)
    {
        data.push_back(255);
        B -= 255;
    }
    data.push_back(mfxU8(B));

    B = size;

    while (B > 255)
    {
        data.push_back(255);
        B -= 255;
    }
    data.push_back(mfxU8(B));
}

mfxU8 Char2Hex(msdk_char ch)
{
    msdk_char value = ch;
    if(value >= MSDK_CHAR('0') && value <= MSDK_CHAR('9'))
    {
        value -= MSDK_CHAR('0');
    }
    else if (value >= MSDK_CHAR('a') && value <= MSDK_CHAR('f'))
    {
        value = value - MSDK_CHAR('a') + 10;
    }
    else if (value >= MSDK_CHAR('A') && value <= MSDK_CHAR('F'))
    {
        value = value - MSDK_CHAR('A') + 10;
    }
    else
    {
        value = 0;
    }
    return (mfxU8)value;
}

namespace {
    int g_trace_level = MSDK_TRACE_LEVEL_INFO;
}

int msdk_trace_get_level() {
    return g_trace_level;
}

void msdk_trace_set_level(int newLevel) {
    g_trace_level = newLevel;
}

bool msdk_trace_is_printable(int level) {
    return g_trace_level >= level;
}

msdk_ostream & operator <<(msdk_ostream & os, MsdkTraceLevel tl) {
    switch (tl)
    {
        case MSDK_TRACE_LEVEL_CRITICAL :
            os<<MSDK_STRING("CRITICAL");
            break;
        case MSDK_TRACE_LEVEL_ERROR :
            os<<MSDK_STRING("ERROR");
            break;
        case MSDK_TRACE_LEVEL_WARNING :
            os<<MSDK_STRING("WARNING");
            break;
        case MSDK_TRACE_LEVEL_INFO :
            os<<MSDK_STRING("INFO");
            break;
        case MSDK_TRACE_LEVEL_DEBUG :
            os<<MSDK_STRING("DEBUG");
            break;
        default:
            break;
    }
    return os;
}

msdk_string NoFullPath(const msdk_string & file_path) {
    size_t pos = file_path.find_last_of(MSDK_STRING("\\/"));
    if (pos != msdk_string::npos) {
        return file_path.substr(pos + 1);
    }
    return file_path;
}

template<> mfxStatus
msdk_opt_read(const msdk_char* string, mfxU8& value)
{
    msdk_char* stopCharacter;
    value = (mfxU8)msdk_strtol(string, &stopCharacter, 10);

    return (msdk_strlen(stopCharacter) == 0)? MFX_ERR_NONE: MFX_ERR_UNKNOWN;
}

template<> mfxStatus
msdk_opt_read(const msdk_char* string, mfxU16& value)
{
    msdk_char* stopCharacter;
    value = (mfxU16)msdk_strtol(string, &stopCharacter, 10);

    return (msdk_strlen(stopCharacter) == 0)? MFX_ERR_NONE: MFX_ERR_UNKNOWN;
}

template<> mfxStatus
msdk_opt_read(const msdk_char* string, mfxU32& value)
{
    msdk_char* stopCharacter;
    value = (mfxU32)msdk_strtol(string, &stopCharacter, 10);

    return (msdk_strlen(stopCharacter) == 0)? MFX_ERR_NONE: MFX_ERR_UNKNOWN;
}

template<> mfxStatus
msdk_opt_read(const msdk_char* string, mfxF32& value)
{
    msdk_char* stopCharacter;
    value = (mfxF32)msdk_strtod(string, &stopCharacter);
    return (msdk_strlen(stopCharacter) == 0)? MFX_ERR_NONE: MFX_ERR_UNKNOWN;
}
template<> mfxStatus
msdk_opt_read(const msdk_char* string, mfxF64& value)
{
    msdk_char* stopCharacter;
    value = (mfxF64)msdk_strtod(string, &stopCharacter);

    return (msdk_strlen(stopCharacter) == 0)? MFX_ERR_NONE: MFX_ERR_UNKNOWN;
}

mfxStatus msdk_opt_read(const msdk_char* string, mfxU8& value);
mfxStatus msdk_opt_read(const msdk_char* string, mfxU16& value);
mfxStatus msdk_opt_read(const msdk_char* string, mfxU32& value);
mfxStatus msdk_opt_read(const msdk_char* string, mfxF64& value);
mfxStatus msdk_opt_read(const msdk_char* string, mfxF32& value);

template<> mfxStatus
msdk_opt_read(const msdk_char* string, mfxI16& value)
{
    msdk_char* stopCharacter;
    value = (mfxI16)msdk_strtol(string, &stopCharacter, 10);

    return (msdk_strlen(stopCharacter) == 0)? MFX_ERR_NONE: MFX_ERR_UNKNOWN;
}

template<> mfxStatus
msdk_opt_read(const msdk_char* string, mfxI32& value)
{
    msdk_char* stopCharacter;
    value = (mfxI32)msdk_strtol(string, &stopCharacter, 10);

    return (msdk_strlen(stopCharacter) == 0)? MFX_ERR_NONE: MFX_ERR_UNKNOWN;
}

mfxStatus msdk_opt_read(const msdk_char* string, mfxI16& value);
mfxStatus msdk_opt_read(const msdk_char* string, mfxI32& value);

template<> mfxStatus
msdk_opt_read(const msdk_char* string, mfxPriority& value)
{
    mfxU32 priority = 0;
    mfxStatus sts = msdk_opt_read<>(string, priority);

    if (MFX_ERR_NONE == sts) value = (mfxPriority)priority;
    return sts;
}

mfxStatus msdk_opt_read(msdk_char* string, mfxPriority& value);

bool IsDecodeCodecSupported(mfxU32 codecFormat)
{
    switch(codecFormat)
    {
        case MFX_CODEC_MPEG2:
        case MFX_CODEC_AVC:
        case MFX_CODEC_HEVC:
        case MFX_CODEC_VC1:
        case CODEC_MVC:
        case MFX_CODEC_JPEG:
        case MFX_CODEC_VP8:
        case MFX_CODEC_VP9:
        case MFX_CODEC_AV1:
        break;
    default:
        return false;
    }
    return true;
}

bool IsEncodeCodecSupported(mfxU32 codecFormat)
{
    switch(codecFormat)
    {
        case MFX_CODEC_AVC:
        case MFX_CODEC_HEVC:
        case MFX_CODEC_MPEG2:
        case CODEC_MVC:
        case MFX_CODEC_VP8:
        case MFX_CODEC_JPEG:
        case MFX_CODEC_VP9:
        break;
    default:
        return false;
    }
    return true;
}

bool IsPluginCodecSupported(mfxU32 codecFormat)
{
    switch(codecFormat)
    {
        case MFX_CODEC_HEVC:
        case MFX_CODEC_AVC:
        case MFX_CODEC_MPEG2:
        case MFX_CODEC_VC1:
        case MFX_CODEC_VP8:
        case MFX_CODEC_VP9:
        break;
    default:
        return false;
    }
    return true;
}

mfxStatus StrFormatToCodecFormatFourCC(msdk_char* strInput, mfxU32 &codecFormat)
{
    mfxStatus sts = MFX_ERR_NONE;
    codecFormat   = 0;

    if (strInput == NULL)
        sts = MFX_ERR_NULL_PTR;

    if (sts == MFX_ERR_NONE)
    {
        if (0 == msdk_strcmp(strInput, MSDK_STRING("mpeg2")))
        {
            codecFormat = MFX_CODEC_MPEG2;
        }
        else if (0 == msdk_strcmp(strInput, MSDK_STRING("h264")))
        {
            codecFormat = MFX_CODEC_AVC;
        }
        else if (0 == msdk_strcmp(strInput, MSDK_STRING("h265")))
        {
            codecFormat = MFX_CODEC_HEVC;
        }
        else if (0 == msdk_strcmp(strInput, MSDK_STRING("vc1")))
        {
            codecFormat = MFX_CODEC_VC1;
        }
        else if (0 == msdk_strcmp(strInput, MSDK_STRING("mvc")))
        {
            codecFormat = CODEC_MVC;
        }
        else if (0 == msdk_strcmp(strInput, MSDK_STRING("jpeg")))
        {
            codecFormat = MFX_CODEC_JPEG;
        }
        else if (0 == msdk_strcmp(strInput, MSDK_STRING("vp8")))
        {
            codecFormat = MFX_CODEC_VP8;
        }
        else if (0 == msdk_strcmp(strInput, MSDK_STRING("vp9")))
        {
            codecFormat = MFX_CODEC_VP9;
        }
        else if (0 == msdk_strcmp(strInput, MSDK_STRING("av1")))
        {
            codecFormat = MFX_CODEC_AV1;
        }
        else if ((0 == msdk_strcmp(strInput, MSDK_STRING("raw"))))
        {
            codecFormat = MFX_CODEC_DUMP;
        }
        else if ((0 == msdk_strcmp(strInput, MSDK_STRING("rgb4_frame"))))
        {
            codecFormat = MFX_CODEC_RGB4;
        }
        else if ((0 == msdk_strcmp(strInput, MSDK_STRING("nv12"))))
        {
            codecFormat = MFX_CODEC_NV12;
        }
        else if ((0 == msdk_strcmp(strInput, MSDK_STRING("i420"))))
        {
            codecFormat = MFX_CODEC_I420;
        }
        else if ((0 == msdk_strcmp(strInput, MSDK_STRING("p010")))) {
            codecFormat = MFX_CODEC_P010;
        }
        else
            sts = MFX_ERR_UNSUPPORTED;
    }

    return sts;
}

msdk_string StatusToString(mfxStatus sts)
{
    switch(sts)
    {
    case MFX_ERR_NONE:
        return msdk_string(MSDK_STRING("MFX_ERR_NONE"));
    case MFX_ERR_UNKNOWN:
        return msdk_string(MSDK_STRING("MFX_ERR_UNKNOWN"));
    case MFX_ERR_NULL_PTR:
        return msdk_string(MSDK_STRING("MFX_ERR_NULL_PTR"));
    case MFX_ERR_UNSUPPORTED:
        return msdk_string(MSDK_STRING("MFX_ERR_UNSUPPORTED"));
    case MFX_ERR_MEMORY_ALLOC:
        return msdk_string(MSDK_STRING("MFX_ERR_MEMORY_ALLOC"));
    case MFX_ERR_NOT_ENOUGH_BUFFER:
        return msdk_string(MSDK_STRING("MFX_ERR_NOT_ENOUGH_BUFFER"));
    case MFX_ERR_INVALID_HANDLE:
        return msdk_string(MSDK_STRING("MFX_ERR_INVALID_HANDLE"));
    case MFX_ERR_LOCK_MEMORY:
        return msdk_string(MSDK_STRING("MFX_ERR_LOCK_MEMORY"));
    case MFX_ERR_NOT_INITIALIZED:
        return msdk_string(MSDK_STRING("MFX_ERR_NOT_INITIALIZED"));
    case MFX_ERR_NOT_FOUND:
        return msdk_string(MSDK_STRING("MFX_ERR_NOT_FOUND"));
    case MFX_ERR_MORE_DATA:
        return msdk_string(MSDK_STRING("MFX_ERR_MORE_DATA"));
    case MFX_ERR_MORE_SURFACE:
        return msdk_string(MSDK_STRING("MFX_ERR_MORE_SURFACE"));
    case MFX_ERR_ABORTED:
        return msdk_string(MSDK_STRING("MFX_ERR_ABORTED"));
    case MFX_ERR_DEVICE_LOST:
        return msdk_string(MSDK_STRING("MFX_ERR_DEVICE_LOST"));
    case MFX_ERR_INCOMPATIBLE_VIDEO_PARAM:
        return msdk_string(MSDK_STRING("MFX_ERR_INCOMPATIBLE_VIDEO_PARAM"));
    case MFX_ERR_INVALID_VIDEO_PARAM:
        return msdk_string(MSDK_STRING("MFX_ERR_INVALID_VIDEO_PARAM"));
    case MFX_ERR_UNDEFINED_BEHAVIOR:
        return msdk_string(MSDK_STRING("MFX_ERR_UNDEFINED_BEHAVIOR"));
    case MFX_ERR_DEVICE_FAILED:
        return msdk_string(MSDK_STRING("MFX_ERR_DEVICE_FAILED"));
    case MFX_ERR_MORE_BITSTREAM:
        return msdk_string(MSDK_STRING("MFX_ERR_MORE_BITSTREAM"));
    case MFX_ERR_INCOMPATIBLE_AUDIO_PARAM:
        return msdk_string(MSDK_STRING("MFX_ERR_INCOMPATIBLE_AUDIO_PARAM"));
    case MFX_ERR_INVALID_AUDIO_PARAM:
        return msdk_string(MSDK_STRING("MFX_ERR_INVALID_AUDIO_PARAM"));
    case MFX_ERR_GPU_HANG:
        return msdk_string(MSDK_STRING("MFX_ERR_GPU_HANG"));
    case MFX_ERR_REALLOC_SURFACE:
        return msdk_string(MSDK_STRING("MFX_ERR_REALLOC_SURFACE"));
    case MFX_WRN_IN_EXECUTION:
        return msdk_string(MSDK_STRING("MFX_WRN_IN_EXECUTION"));
    case MFX_WRN_DEVICE_BUSY:
        return msdk_string(MSDK_STRING("MFX_WRN_DEVICE_BUSY"));
    case MFX_WRN_VIDEO_PARAM_CHANGED:
        return msdk_string(MSDK_STRING("MFX_WRN_VIDEO_PARAM_CHANGED"));
    case MFX_WRN_PARTIAL_ACCELERATION:
        return msdk_string(MSDK_STRING("MFX_WRN_PARTIAL_ACCELERATION"));
    case MFX_WRN_INCOMPATIBLE_VIDEO_PARAM:
        return msdk_string(MSDK_STRING("MFX_WRN_INCOMPATIBLE_VIDEO_PARAM"));
    case MFX_WRN_VALUE_NOT_CHANGED:
        return msdk_string(MSDK_STRING("MFX_WRN_VALUE_NOT_CHANGED"));
    case MFX_WRN_OUT_OF_RANGE:
        return msdk_string(MSDK_STRING("MFX_WRN_OUT_OF_RANGE"));
    case MFX_WRN_FILTER_SKIPPED:
        return msdk_string(MSDK_STRING("MFX_WRN_FILTER_SKIPPED"));
    case MFX_WRN_INCOMPATIBLE_AUDIO_PARAM:
        return msdk_string(MSDK_STRING("MFX_WRN_INCOMPATIBLE_AUDIO_PARAM"));
    case MFX_TASK_WORKING:
        return msdk_string(MSDK_STRING("MFX_TASK_WORKING"));
    case MFX_TASK_BUSY:
        return msdk_string(MSDK_STRING("MFX_TASK_BUSY"));
    case MFX_ERR_MORE_DATA_SUBMIT_TASK:
        return msdk_string(MSDK_STRING("MFX_ERR_MORE_DATA_SUBMIT_TASK"));
    default:
        return msdk_string(MSDK_STRING("[Unknown status]"));
    }
}

mfxI32 getMonitorType(msdk_char* str)
{
    struct {
      const msdk_char* str;
      mfxI32 mfx_type;
    } table[] = {
#define __DECLARE(type) { MSDK_STRING(#type), MFX_MONITOR_ ## type }
      __DECLARE(Unknown),
      __DECLARE(VGA),
      __DECLARE(DVII),
      __DECLARE(DVID),
      __DECLARE(DVIA),
      __DECLARE(Composite),
      __DECLARE(SVIDEO),
      __DECLARE(LVDS),
      __DECLARE(Component),
      __DECLARE(9PinDIN),
      __DECLARE(HDMIA),
      __DECLARE(HDMIB),
      __DECLARE(eDP),
      __DECLARE(TV),
      __DECLARE(DisplayPort),
#if defined(DRM_MODE_CONNECTOR_VIRTUAL) // from libdrm 2.4.59
      __DECLARE(VIRTUAL),
#endif
#if defined(DRM_MODE_CONNECTOR_DSI) // from libdrm 2.4.59
      __DECLARE(DSI)
#endif
#undef __DECLARE
    };
    for (unsigned int i=0; i < sizeof(table)/sizeof(table[0]); ++i) {
      if (0 == msdk_strcmp(str, table[i].str)) {
        return table[i].mfx_type;
      }
    }
    return MFX_MONITOR_MAXNUMBER;
}

CH264FrameReader::CH264FrameReader()
: CSmplBitstreamReader()
, m_processedBS(0)
, m_isEndOfStream(false)
, m_frame(0)
, m_plainBuffer(0)
, m_plainBufferSize(0)
{
}

CH264FrameReader::~CH264FrameReader()
{
}

void CH264FrameReader::Close()
{
    CSmplBitstreamReader::Close();

    if (NULL != m_plainBuffer)
    {
        free(m_plainBuffer);
        m_plainBuffer = NULL;
        m_plainBufferSize = 0;
    }
}

mfxStatus CH264FrameReader::Init(const msdk_char *strFileName)
{
    mfxStatus sts = MFX_ERR_NONE;

    sts = CSmplBitstreamReader::Init(strFileName);
    if (sts != MFX_ERR_NONE)
        return sts;

    m_isEndOfStream = false;
    m_processedBS = NULL;

    m_originalBS.Extend(1024 * 1024);

    m_pNALSplitter.reset(new ProtectedLibrary::AVC_Spl());

    m_frame = 0;
    m_plainBuffer = 0;
    m_plainBufferSize = 0;

    return sts;
}

mfxStatus CH264FrameReader::ReadNextFrame(mfxBitstream *pBS)
{
    mfxStatus sts = MFX_ERR_NONE;
    pBS->DataFlag = MFX_BITSTREAM_COMPLETE_FRAME;
    //read bit stream from source
    while (!m_originalBS.DataLength)
    {
        sts = CSmplBitstreamReader::ReadNextFrame(&m_originalBS);
        if (sts != MFX_ERR_NONE && sts != MFX_ERR_MORE_DATA)
            return sts;
        if (sts == MFX_ERR_MORE_DATA)
        {
            m_isEndOfStream = true;
            break;
        }
    }

    do
    {
        sts = PrepareNextFrame(m_isEndOfStream ? NULL : &m_originalBS, &m_processedBS);

        if (sts == MFX_ERR_MORE_DATA)
        {
            if (m_isEndOfStream)
            {
                break;
            }

            sts = CSmplBitstreamReader::ReadNextFrame(&m_originalBS);
            if (sts == MFX_ERR_MORE_DATA)
                m_isEndOfStream = true;
            continue;
        }
        else if (MFX_ERR_NONE != sts)
            return sts;

    } while (MFX_ERR_NONE != sts);

    // get output stream
    if (NULL != m_processedBS)
    {
        mfxStatus copySts = CopyBitstream2(
            pBS,
            m_processedBS);
        if (copySts < MFX_ERR_NONE)
            return copySts;
        m_processedBS = NULL;
    }

    return sts;
}

mfxStatus CH264FrameReader::PrepareNextFrame(mfxBitstream *in, mfxBitstream **out)
{
    mfxStatus sts = MFX_ERR_NONE;

    if (NULL == out)
        return MFX_ERR_NULL_PTR;

    *out = NULL;

    // get frame if it is not ready yet
    if (NULL == m_frame)
    {
        sts = m_pNALSplitter->GetFrame(in, &m_frame);
        if (sts != MFX_ERR_NONE)
            return sts;
    }

    if (m_plainBufferSize < m_frame->DataLength)
    {
        if (NULL != m_plainBuffer)
        {
            free(m_plainBuffer);
            m_plainBuffer = NULL;
            m_plainBufferSize = 0;
        }
        m_plainBuffer = (mfxU8*)malloc(m_frame->DataLength);
        if (NULL == m_plainBuffer)
            return MFX_ERR_MEMORY_ALLOC;
        m_plainBufferSize = m_frame->DataLength;
    }

    MSDK_MEMCPY_BUF(m_plainBuffer, 0, m_plainBufferSize, m_frame->Data, m_frame->DataLength);

    memset(&m_outBS, 0, sizeof(mfxBitstream));
    m_outBS.Data = m_plainBuffer;
    m_outBS.DataOffset = 0;
    m_outBS.DataLength = m_frame->DataLength;
    m_outBS.MaxLength = m_frame->DataLength;
    m_outBS.DataFlag = MFX_BITSTREAM_COMPLETE_FRAME;
    m_outBS.TimeStamp = m_frame->TimeStamp;

    m_pNALSplitter->ResetCurrentState();
    m_frame = NULL;

    *out = &m_outBS;

    return sts;
}

// This function either performs synchronization using provided syncpoint,
// or just waits for predefined time if no available syncpoint
void WaitForDeviceToBecomeFree(MFXVideoSession& session, mfxSyncPoint& syncPoint, mfxStatus& currentStatus)
{
    if (syncPoint)
    {
        mfxStatus stsSync = session.SyncOperation(syncPoint, MSDK_WAIT_INTERVAL);
        if (MFX_ERR_NONE == stsSync)
        {
            // Retire completed sync point (otherwise we may start active polling)
            syncPoint = NULL;
            currentStatus = MFX_ERR_NONE;
        }
        else
        {
            MSDK_TRACE_ERROR(MSDK_STRING("WaitForDeviceToBecomeFree: SyncOperation failed, sts = ") << stsSync);
            currentStatus = MFX_ERR_ABORTED;
        }
    }
    else
    {
        MSDK_SLEEP(1);
        currentStatus = MFX_ERR_NONE;
    }
}

mfxU16 FourCCToChroma(mfxU32 fourCC)
{
    switch(fourCC)
    {
    case MFX_FOURCC_NV12:
    case MFX_FOURCC_P010:
#if (MFX_VERSION >= 1031)
    case MFX_FOURCC_P016:
#endif
        return MFX_CHROMAFORMAT_YUV420;
    case MFX_FOURCC_NV16:
    case MFX_FOURCC_P210:
#if (MFX_VERSION >= 1027)
    case MFX_FOURCC_Y210:
#endif
#if (MFX_VERSION >= 1031)
    case MFX_FOURCC_Y216:
#endif
    case MFX_FOURCC_YUY2:
    case MFX_FOURCC_UYVY:
        return MFX_CHROMAFORMAT_YUV422;
#if (MFX_VERSION >= 1027)
    case MFX_FOURCC_Y410:
    case MFX_FOURCC_A2RGB10:
#endif
    case MFX_FOURCC_AYUV:
    case MFX_FOURCC_RGB4:
        return MFX_CHROMAFORMAT_YUV444;
    }

    return MFX_CHROMAFORMAT_YUV420;
}
