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

#ifndef __SAMPLE_UTILS_H__
#define __SAMPLE_UTILS_H__

#include <stdio.h>
#include <string>
#include <sstream>
#include <vector>
#include <map>
#include <stdexcept>
#include <mutex>
#include <algorithm>
#include <fstream>

#include "mfxstructures.h"
#include "mfxvideo.h"
#include "mfxvideo++.h"
#include "mfxjpeg.h"
#include "mfxplugin.h"
#include "mfxbrc.h"
#include "mfxfei.h"
#include "mfxfeihevc.h"
#include "mfxmvc.h"
#include "mfxla.h"

#include "vm/strings_defs.h"
#include "vm/file_defs.h"
#include "vm/time_defs.h"
#include "vm/atomic_defs.h"
#include "vm/thread_defs.h"

#include "sample_types.h"

#include "abstract_splitter.h"
#include "avc_bitstream.h"
#include "avc_spl.h"
#include "avc_headers.h"
#include "avc_nal_spl.h"


// A macro to disallow the copy constructor and operator= functions
// This should be used in the private: declarations for a class
#define DISALLOW_COPY_AND_ASSIGN(TypeName) \
    TypeName(const TypeName&);               \
    void operator=(const TypeName&)

//! Base class for types that should not be assigned.
class no_assign {
    // Deny assignment
    void operator=( const no_assign& );
public:
#if __GNUC__
    //! Explicitly define default construction, because otherwise gcc issues gratuitous warning.
    no_assign() {}
#endif /* __GNUC__ */
};

//! Base class for types that should not be copied or assigned.
class no_copy: no_assign {
    //! Deny copy construction
    no_copy( const no_copy& );
public:
    //! Allow default construction
    no_copy() {}
};

enum {
    CODEC_VP8 = MFX_MAKEFOURCC('V','P','8',' '),
    CODEC_MVC = MFX_MAKEFOURCC('M','V','C',' '),
};

#define MFX_CODEC_DUMP MFX_MAKEFOURCC('D','U','M','P')
#define MFX_CODEC_RGB4 MFX_FOURCC_RGB4
#define MFX_CODEC_NV12 MFX_FOURCC_NV12
#define MFX_CODEC_I420 MFX_FOURCC_I420
#define MFX_CODEC_P010 MFX_FOURCC_P010

enum
{
    MFX_FOURCC_IMC3         = MFX_MAKEFOURCC('I','M','C','3'),
    MFX_FOURCC_YUV400       = MFX_MAKEFOURCC('4','0','0','P'),
    MFX_FOURCC_YUV411       = MFX_MAKEFOURCC('4','1','1','P'),
    MFX_FOURCC_YUV422H      = MFX_MAKEFOURCC('4','2','2','H'),
    MFX_FOURCC_YUV422V      = MFX_MAKEFOURCC('4','2','2','V'),
    MFX_FOURCC_YUV444       = MFX_MAKEFOURCC('4','4','4','P'),
#if (MFX_VERSION <= 1027)
    MFX_FOURCC_RGBP         = MFX_MAKEFOURCC('R','G','B','P'),
#endif
    MFX_FOURCC_I420         = MFX_MAKEFOURCC('I','4','2','0')
};

enum ExtBRCType {
    EXTBRC_DEFAULT,
    EXTBRC_OFF,
    EXTBRC_ON,
    EXTBRC_IMPLICIT
};

namespace QPFile {

    enum ReaderStatus
    {
        READER_ERR_NONE,
        READER_ERR_NOT_INITIALIZED,
        READER_ERR_CODEC_UNSUPPORTED,
        READER_ERR_FILE_NOT_OPEN,
        READER_ERR_INCORRECT_FILE
    };

    struct FrameInfo
    {
        mfxU32 displayOrder;
        mfxU16 QP;
        mfxU16 frameType;
    };

    // QPFile::Reader reads QP and frame type per frame in encoding order
    // from external text file (for encoding in qpfile mode)
    class Reader
    {
    public:
        mfxStatus Read(const msdk_string& strFileName, mfxU32 codecid);
        void ResetState();

        mfxU32 GetCurrentEncodedOrder() const;
        mfxU32 GetCurrentDisplayOrder() const;
        mfxU16 GetCurrentQP() const;
        mfxU16 GetCurrentFrameType() const;
        mfxU32 GetFramesNum() const;
        void NextFrame();
        std::string GetErrorMessage() const;

    private:
        void ResetState(ReaderStatus set_sts);

        ReaderStatus            m_ReaderSts   = READER_ERR_NOT_INITIALIZED;
        mfxU32                  m_nFrames     = std::numeric_limits<mfxU32>::max();
        mfxU32                  m_CurFrameNum = std::numeric_limits<mfxU32>::max();
        std::vector<FrameInfo>  m_FrameVals {};
    };

    inline bool get_line(std::ifstream& ifs, std::string& line)
    {
        std::getline(ifs, line, '\n');
        if (!line.empty() && line.back() == '\r')
            line.pop_back();
        return !ifs.fail();
    }
    inline size_t find_nth(const std::string& str, size_t pos, const std::string& needle, mfxU32 nth)
    {
        size_t found_pos = str.find(needle, pos);
        for(; nth != 0 && std::string::npos != found_pos; --nth)
            found_pos = str.find(needle, found_pos + 1);
        return found_pos;
    }
    inline mfxU16 StringToFrameType(std::string str)
    {
        if ("IDR_REF" == str) return MFX_FRAMETYPE_I | MFX_FRAMETYPE_IDR | MFX_FRAMETYPE_REF;
        else if ("I_REF" == str) return MFX_FRAMETYPE_I | MFX_FRAMETYPE_REF;
        else if ("P_REF" == str) return MFX_FRAMETYPE_P | MFX_FRAMETYPE_REF;
        else if ("P" == str) return MFX_FRAMETYPE_P;
        else if ("B_REF" == str) return MFX_FRAMETYPE_B | MFX_FRAMETYPE_REF;
        else if ("B" == str) return MFX_FRAMETYPE_B;
        else return MFX_FRAMETYPE_UNKNOWN;
    }
    inline std::string ReaderStatusToString(ReaderStatus sts)
    {
        switch (sts)
        {
        case READER_ERR_NOT_INITIALIZED:
            return std::string("reader not initialized (qpfile has not yet read the file)\n");
        case READER_ERR_FILE_NOT_OPEN:
            return std::string("failed to open file contains frame parameters (check provided path in -qpfile <path>)\n");
        case READER_ERR_INCORRECT_FILE:
            return std::string("incorrect file with frame parameters\n");
        case READER_ERR_CODEC_UNSUPPORTED:
            return std::string("codecs, except h264 and h265, are not supported\n");
        default:
            return std::string();
        }
    }
    inline mfxU32 ReadDisplayOrder(const std::string& line)
    {
        return std::stoi(line.substr(0, find_nth(line, 0, ",", 0)));
    }
    inline mfxU16 ReadQP(const std::string& line)
    {
        size_t pos = find_nth(line, 0, ",", 0) + 1;
        return static_cast<mfxU16>(std::stoi(line.substr(pos, find_nth(line, 0, ",", 1) - pos)));
    }
    inline mfxU16 ReadFrameType(const std::string& line)
    {
        size_t pos = find_nth(line, 0, ",", 1) + 1;
        return StringToFrameType(line.substr(pos, line.length() - pos));
    }
}

namespace TCBRCTestFile {

    enum ReaderStatus
    {
        READER_ERR_NONE,
        READER_ERR_NOT_INITIALIZED,
        READER_ERR_CODEC_UNSUPPORTED,
        READER_ERR_FILE_NOT_OPEN,
        READER_ERR_INCORRECT_FILE
    };

    struct FrameInfo
    {
        mfxU32 displayOrder;
        mfxU32 targetFrameSize;
    };

    // TCBRCTestFile reads target frame size in display order
    // from external text file (for encoding in Low delay BRC mode)

    class Reader
    {
    public:
        mfxStatus Read(const msdk_string& strFileName, mfxU32 codecid);
        void ResetState();

        mfxU32 GetTargetFrameSize(mfxU32 frameOrder) const;
        mfxU32 GetFramesNum() const;
        void NextFrame();
        std::string GetErrorMessage() const;

    private:
        void ResetState(ReaderStatus set_sts);

        ReaderStatus            m_ReaderSts = READER_ERR_NOT_INITIALIZED;
        mfxU32                  m_CurFrameNum = std::numeric_limits<mfxU32>::max();
        std::vector<FrameInfo>  m_FrameVals{};
    };

    inline bool get_line(std::ifstream& ifs, std::string& line)
    {
        std::getline(ifs, line, '\n');
        if (!line.empty() && line.back() == '\r')
            line.pop_back();
        return !ifs.fail();
    }
    inline size_t find_nth(const std::string& str, size_t pos, const std::string& needle, mfxU32 nth)
    {
        size_t found_pos = str.find(needle, pos);
        for (; nth != 0 && std::string::npos != found_pos; --nth)
            found_pos = str.find(needle, found_pos + 1);
        return found_pos;
    }

    inline std::string ReaderStatusToString(ReaderStatus sts)
    {
        switch (sts)
        {
        case READER_ERR_NOT_INITIALIZED:
            return std::string("reader not initialized (TCBRCTestfile has not yet read the file)\n");
        case READER_ERR_FILE_NOT_OPEN:
            return std::string("failed to open file  with TargetFrameSize parameters (check provided path in -tcbrcfile <path>)\n");
        case READER_ERR_INCORRECT_FILE:
            return std::string("incorrect file with frame parameters\n");
        case READER_ERR_CODEC_UNSUPPORTED:
            return std::string("h264 and h265 are supported now\n");
        default:
            return std::string();
        }
    }
    inline mfxU32 ReadDisplayOrder(const std::string& line)
    {
        size_t pos = find_nth(line, 0, ":", 0);
        if (pos != std::string::npos)
            return std::stoi(line.substr(0, pos));
        else
            return 0;
    }
    inline mfxU16 ReadTargetFrameSize(const std::string& line)
    {
        size_t pos = find_nth(line, 0, ":", 0);
        pos = (pos != std::string::npos) ? pos + 1 : 0;
        return static_cast<mfxU16>(std::stoi(line.substr(pos, line.size() - pos)));
    }
}
mfxStatus GetFrameLength(mfxU16 width, mfxU16 height, mfxU32 ColorFormat, mfxU32 &length);

bool IsDecodeCodecSupported(mfxU32 codecFormat);
bool IsEncodeCodecSupported(mfxU32 codecFormat);
bool IsPluginCodecSupported(mfxU32 codecFormat);

// class is used as custom exception
class mfxError : public std::runtime_error
{
public:
    mfxError(mfxStatus status = MFX_ERR_UNKNOWN, std::string msg = "")
        : runtime_error(msg)
        , m_Status(status)
    {}

    mfxStatus GetStatus() const
    { return m_Status; }

private:
    mfxStatus m_Status;
};

//declare used extension buffers
template<class T>
struct mfx_ext_buffer_id{};

template<>struct mfx_ext_buffer_id<mfxExtCodingOption>{
    enum {id = MFX_EXTBUFF_CODING_OPTION};
};
template<>struct mfx_ext_buffer_id<mfxExtCodingOption2>{
    enum {id = MFX_EXTBUFF_CODING_OPTION2};
};
template<>struct mfx_ext_buffer_id<mfxExtCodingOption3>{
    enum {id = MFX_EXTBUFF_CODING_OPTION3};
};
template<>struct mfx_ext_buffer_id<mfxExtAvcTemporalLayers>{
    enum {id = MFX_EXTBUFF_AVC_TEMPORAL_LAYERS};
};
template<>struct mfx_ext_buffer_id<mfxExtAVCRefListCtrl>{
    enum {id = MFX_EXTBUFF_AVC_REFLIST_CTRL};
};
template<>struct mfx_ext_buffer_id<mfxExtThreadsParam>{
    enum {id = MFX_EXTBUFF_THREADS_PARAM};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiParam> {
    enum {id = MFX_EXTBUFF_FEI_PARAM};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiPreEncCtrl> {
    enum {id = MFX_EXTBUFF_FEI_PREENC_CTRL};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiPreEncMV>{
    enum {id = MFX_EXTBUFF_FEI_PREENC_MV};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiPreEncMBStat>{
    enum {id = MFX_EXTBUFF_FEI_PREENC_MB};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiHevcEncFrameCtrl>{
    enum {id = MFX_EXTBUFF_HEVCFEI_ENC_CTRL};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiHevcEncMVPredictors>{
    enum {id = MFX_EXTBUFF_HEVCFEI_ENC_MV_PRED};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiHevcEncQP>{
    enum {id = MFX_EXTBUFF_HEVCFEI_ENC_QP};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiHevcEncCtuCtrl>{
    enum {id = MFX_EXTBUFF_HEVCFEI_ENC_CTU_CTRL};
};
template<>struct mfx_ext_buffer_id<mfxExtHEVCRefLists>{
    enum {id = MFX_EXTBUFF_HEVC_REFLISTS};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiHevcRepackCtrl>{
    enum {id = MFX_EXTBUFF_HEVCFEI_REPACK_CTRL};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiHevcRepackStat>{
    enum {id = MFX_EXTBUFF_HEVCFEI_REPACK_STAT};
};
template<>struct mfx_ext_buffer_id<mfxExtBRC> {
    enum {id = MFX_EXTBUFF_BRC};
};
template<>struct mfx_ext_buffer_id<mfxExtHEVCParam> {
    enum {id = MFX_EXTBUFF_HEVC_PARAM};
};
template<>struct mfx_ext_buffer_id<mfxExtDecVideoProcessing> {
    enum {id = MFX_EXTBUFF_DEC_VIDEO_PROCESSING};
};
template<>struct mfx_ext_buffer_id<mfxExtDecodeErrorReport> {
    enum {id = MFX_EXTBUFF_DECODE_ERROR_REPORT};
};
template<>struct mfx_ext_buffer_id<mfxExtMVCSeqDesc> {
    enum {id = MFX_EXTBUFF_MVC_SEQ_DESC};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPDoNotUse> {
    enum {id = MFX_EXTBUFF_VPP_DONOTUSE};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPDoUse> {
    enum {id = MFX_EXTBUFF_VPP_DOUSE};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPDeinterlacing> {
    enum {id = MFX_EXTBUFF_VPP_DEINTERLACING};
};
template<>struct mfx_ext_buffer_id<mfxExtCodingOptionSPSPPS> {
    enum {id = MFX_EXTBUFF_CODING_OPTION_SPSPPS};
};
template<>struct mfx_ext_buffer_id<mfxExtOpaqueSurfaceAlloc> {
    enum {id = MFX_EXTBUFF_OPAQUE_SURFACE_ALLOCATION};
};
template<>struct mfx_ext_buffer_id<mfxExtVppMctf> {
    enum {id = MFX_EXTBUFF_VPP_MCTF};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPComposite> {
    enum {id = MFX_EXTBUFF_VPP_COMPOSITE};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPFieldProcessing> {
    enum {id = MFX_EXTBUFF_VPP_FIELD_PROCESSING};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPDetail> {
    enum {id = MFX_EXTBUFF_VPP_DETAIL};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPFrameRateConversion> {
    enum {id = MFX_EXTBUFF_VPP_FRAME_RATE_CONVERSION};
};
template<>struct mfx_ext_buffer_id<mfxExtLAControl> {
    enum {id = MFX_EXTBUFF_LOOKAHEAD_CTRL};
};
template<>struct mfx_ext_buffer_id<mfxExtMultiFrameControl> {
    enum {id = MFX_EXTBUFF_MULTI_FRAME_CONTROL};
};
template<>struct mfx_ext_buffer_id<mfxExtMultiFrameParam> {
    enum {id = MFX_EXTBUFF_MULTI_FRAME_PARAM};
};
template<>struct mfx_ext_buffer_id<mfxExtHEVCTiles> {
    enum {id = MFX_EXTBUFF_HEVC_TILES};
};
template<>struct mfx_ext_buffer_id<mfxExtVP9Param> {
    enum {id = MFX_EXTBUFF_VP9_PARAM};
};
template<>struct mfx_ext_buffer_id<mfxExtVideoSignalInfo> {
    enum {id = MFX_EXTBUFF_VIDEO_SIGNAL_INFO};
};
template<>struct mfx_ext_buffer_id<mfxExtHEVCRegion> {
    enum {id = MFX_EXTBUFF_HEVC_REGION};
};
template<>struct mfx_ext_buffer_id<mfxExtAVCRoundingOffset> {
    enum {id = MFX_EXTBUFF_AVC_ROUNDING_OFFSET};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPDenoise> {
    enum {id = MFX_EXTBUFF_VPP_DENOISE};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPProcAmp> {
    enum {id = MFX_EXTBUFF_VPP_PROCAMP};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPImageStab> {
    enum {id = MFX_EXTBUFF_VPP_IMAGE_STABILIZATION};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPVideoSignalInfo> {
    enum {id = MFX_EXTBUFF_VPP_VIDEO_SIGNAL_INFO};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPMirroring> {
    enum {id = MFX_EXTBUFF_VPP_MIRRORING};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPColorFill> {
    enum {id = MFX_EXTBUFF_VPP_COLORFILL};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPRotation> {
    enum {id = MFX_EXTBUFF_VPP_ROTATION};
};
template<>struct mfx_ext_buffer_id<mfxExtVPPScaling> {
    enum {id = MFX_EXTBUFF_VPP_SCALING};
};
template<>struct mfx_ext_buffer_id<mfxExtColorConversion> {
    enum {id = MFX_EXTBUFF_VPP_COLOR_CONVERSION};
};
template<>struct mfx_ext_buffer_id<mfxExtPredWeightTable> {
    enum {id = MFX_EXTBUFF_PRED_WEIGHT_TABLE};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiDecStreamOut> {
    enum {id = MFX_EXTBUFF_FEI_DEC_STREAM_OUT};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiSliceHeader> {
    enum {id = MFX_EXTBUFF_FEI_SLICE};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiEncFrameCtrl> {
    enum {id = MFX_EXTBUFF_FEI_ENC_CTRL};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiEncMVPredictors> {
    enum {id = MFX_EXTBUFF_FEI_ENC_MV_PRED};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiRepackCtrl> {
    enum {id = MFX_EXTBUFF_FEI_REPACK_CTRL};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiEncMBCtrl> {
    enum {id = MFX_EXTBUFF_FEI_ENC_MB};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiEncQP> {
    enum {id = MFX_EXTBUFF_FEI_ENC_QP};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiEncMBStat> {
    enum {id = MFX_EXTBUFF_FEI_ENC_MB_STAT};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiEncMV> {
    enum {id = MFX_EXTBUFF_FEI_ENC_MV};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiPakMBCtrl> {
    enum {id = MFX_EXTBUFF_FEI_PAK_CTRL};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiRepackStat> {
    enum {id = MFX_EXTBUFF_FEI_REPACK_STAT};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiSPS> {
    enum {id = MFX_EXTBUFF_FEI_SPS};
};
template<>struct mfx_ext_buffer_id<mfxExtFeiPPS > {
    enum {id = MFX_EXTBUFF_FEI_PPS};
};

constexpr uint16_t max_num_ext_buffers = 63 * 2; // '*2' is for max estimation if all extBuffer were 'paired'

//helper function to initialize mfx ext buffer structure
template <class T>
void init_ext_buffer(T & ext_buffer)
{
    memset(&ext_buffer, 0, sizeof(ext_buffer));
    reinterpret_cast<mfxExtBuffer*>(&ext_buffer)->BufferId = mfx_ext_buffer_id<T>::id;
    reinterpret_cast<mfxExtBuffer*>(&ext_buffer)->BufferSz = sizeof(ext_buffer);
}

template <typename T> struct IsPairedMfxExtBuffer                 : std::false_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtAVCRefListCtrl>     : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtAVCRoundingOffset>  : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtPredWeightTable>    : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiSliceHeader>     : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiEncFrameCtrl>    : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiEncMVPredictors> : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiRepackCtrl>      : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiEncMBCtrl>       : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiEncQP>           : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiEncMBStat>       : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiEncMV>           : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiPakMBCtrl>       : std::true_type {};
template <> struct IsPairedMfxExtBuffer<mfxExtFeiRepackStat>      : std::true_type {};

template <typename R>
struct ExtParamAccessor
{
private:
    using mfxExtBufferDoublePtr = mfxExtBuffer**;
public:
    mfxU16& NumExtParam;
    mfxExtBufferDoublePtr& ExtParam;
    ExtParamAccessor(const R& r):
        NumExtParam(const_cast<mfxU16&>(r.NumExtParam)),
        ExtParam(const_cast<mfxExtBufferDoublePtr&>(r.ExtParam)) {}
};

template <>
struct ExtParamAccessor<mfxFrameSurface1>
{
private:
    using mfxExtBufferDoublePtr = mfxExtBuffer**;
public:
    mfxU16& NumExtParam;
    mfxExtBufferDoublePtr& ExtParam;
    ExtParamAccessor(const mfxFrameSurface1& r):
        NumExtParam(const_cast<mfxU16&>(r.Data.NumExtParam)),
        ExtParam(const_cast<mfxExtBufferDoublePtr&>(r.Data.ExtParam)) {}
};

/** ExtBufHolder is an utility class which
 *  provide interface for mfxExtBuffer objects management in any mfx structure (e.g. mfxVideoParam)
 */
template<typename T>
class ExtBufHolder : public T
{
public:
    ExtBufHolder() : T()
    {
        m_ext_buf.reserve(max_num_ext_buffers);
    }

    ~ExtBufHolder() // only buffers allocated by wrapper can be released
    {
        for (auto it = m_ext_buf.begin(); it != m_ext_buf.end(); it++ )
        {
            delete [] (mfxU8*)(*it);
        }
    }

    ExtBufHolder(const ExtBufHolder& ref)
    {
        m_ext_buf.reserve(max_num_ext_buffers);
        *this = ref; // call to operator=
    }

    ExtBufHolder& operator=(const ExtBufHolder& ref)
    {
        const T* src_base = &ref;
        return operator=(*src_base);
    }

    ExtBufHolder(const T& ref)
    {
        *this = ref; // call to operator=
    }

    ExtBufHolder& operator=(const T& ref)
    {
        // copy content of main structure type T
        T* dst_base = this;
        const T* src_base = &ref;
        *dst_base = *src_base;

        //remove all existing extension buffers
        ClearBuffers();

        const auto ref_ = ExtParamAccessor<T>(ref);

        //reproduce list of extension buffers and copy its content
        for (size_t i = 0; i < ref_.NumExtParam; ++i)
        {
            const auto src_buf = ref_.ExtParam[i];
            if (!src_buf) throw mfxError(MFX_ERR_NULL_PTR, "Null pointer attached to source ExtParam");
            if (!IsCopyAllowed(src_buf->BufferId))
            {
                auto msg = "Deep copy of '" + Fourcc2Str(src_buf->BufferId) + "' extBuffer is not allowed";
                throw mfxError(MFX_ERR_UNDEFINED_BEHAVIOR, msg);
            }

            // 'false' below is because here we just copy extBuffer's one by one
            auto dst_buf = AddExtBuffer(src_buf->BufferId, src_buf->BufferSz, false);
            // copy buffer content w/o restoring its type
            memcpy((void*)dst_buf, (void*)src_buf, src_buf->BufferSz);
        }

        return *this;
    }

    ExtBufHolder(ExtBufHolder &&)             = default;
    ExtBufHolder & operator= (ExtBufHolder&&) = default;

    // Always returns a valid pointer or throws an exception
    template<typename TB>
    TB* AddExtBuffer()
    {
        mfxExtBuffer* b = AddExtBuffer(mfx_ext_buffer_id<TB>::id, sizeof(TB), IsPairedMfxExtBuffer<TB>::value);
        return (TB*)b;
    }

    template<typename TB>
    void RemoveExtBuffer()
    {
        auto it = std::find_if(m_ext_buf.begin(), m_ext_buf.end(), CmpExtBufById(mfx_ext_buffer_id<TB>::id));
        if (it != m_ext_buf.end())
        {
            delete [] (mfxU8*)(*it);
            it = m_ext_buf.erase(it);

            if (IsPairedMfxExtBuffer<TB>::value)
            {
                if (it == m_ext_buf.end() || (*it)->BufferId != mfx_ext_buffer_id<TB>::id)
                    throw mfxError(MFX_ERR_NULL_PTR, "RemoveExtBuffer: ExtBuffer's parity has been broken");

                delete [] (mfxU8*)(*it);
                m_ext_buf.erase(it);
            }

            RefreshBuffers();
        }
    }

    template <typename TB>
    TB* GetExtBuffer(uint32_t fieldId = 0) const
    {
        return (TB*)FindExtBuffer(mfx_ext_buffer_id<TB>::id, fieldId);
    }

    template <typename TB>
    operator TB*()
    {
        return (TB*)FindExtBuffer(mfx_ext_buffer_id<TB>::id, 0);
    }

    template <typename TB>
    operator TB*() const
    {
        return (TB*)FindExtBuffer(mfx_ext_buffer_id<TB>::id, 0);
    }

private:

    mfxExtBuffer* AddExtBuffer(mfxU32 id, mfxU32 size, bool isPairedExtBuffer)
    {
        if (!size || !id)
            throw mfxError(MFX_ERR_NULL_PTR, "AddExtBuffer: wrong size or id!");

        auto it = std::find_if(m_ext_buf.begin(), m_ext_buf.end(), CmpExtBufById(id));
        if (it == m_ext_buf.end())
        {
            auto buf = (mfxExtBuffer*)new mfxU8[size];
            memset(buf, 0, size);
            m_ext_buf.push_back(buf);

            buf->BufferId = id;
            buf->BufferSz = size;

            if (isPairedExtBuffer)
            {
                // Allocate the other mfxExtBuffer _right_after_ the first one ...
                buf = (mfxExtBuffer*)new mfxU8[size];
                memset(buf, 0, size);
                m_ext_buf.push_back(buf);

                buf->BufferId = id;
                buf->BufferSz = size;

                RefreshBuffers();
                return m_ext_buf[m_ext_buf.size() - 2]; // ... and return a pointer to the first one
            }

            RefreshBuffers();
            return m_ext_buf.back();
        }

        return *it;
    }

    mfxExtBuffer* FindExtBuffer(mfxU32 id, uint32_t fieldId) const
    {
        auto it = std::find_if(m_ext_buf.begin(), m_ext_buf.end(), CmpExtBufById(id));
        if (fieldId && it != m_ext_buf.end())
        {
            ++it;
            return it != m_ext_buf.end() ? *it : nullptr;
        }
        return it != m_ext_buf.end() ? *it : nullptr;
    }

    void RefreshBuffers()
    {
        auto this_ = ExtParamAccessor<T>(*this);
        this_.NumExtParam = static_cast<mfxU16>(m_ext_buf.size());
        this_.ExtParam    = this_.NumExtParam ? m_ext_buf.data() : nullptr;
    }

    void ClearBuffers()
    {
        if (m_ext_buf.size())
        {
            for (auto it = m_ext_buf.begin(); it != m_ext_buf.end(); it++ )
            {
                delete [] (mfxU8*)(*it);
            }
            m_ext_buf.clear();
        }
        RefreshBuffers();
    }

    bool IsCopyAllowed(mfxU32 id)
    {
        static const mfxU32 allowed[] = {
            MFX_EXTBUFF_CODING_OPTION,
            MFX_EXTBUFF_CODING_OPTION2,
            MFX_EXTBUFF_CODING_OPTION3,
            MFX_EXTBUFF_FEI_PARAM,
            MFX_EXTBUFF_BRC,
            MFX_EXTBUFF_HEVC_PARAM,
            MFX_EXTBUFF_VP9_PARAM,
            MFX_EXTBUFF_OPAQUE_SURFACE_ALLOCATION,
            MFX_EXTBUFF_FEI_PPS,
            MFX_EXTBUFF_FEI_SPS,
            MFX_EXTBUFF_LOOKAHEAD_CTRL,
            MFX_EXTBUFF_LOOKAHEAD_STAT,
            MFX_EXTBUFF_DEC_VIDEO_PROCESSING
        };

        auto it = std::find_if(std::begin(allowed), std::end(allowed),
                               [&id](const mfxU32 allowed_id)
                               {
                                   return allowed_id == id;
                               });
        return it != std::end(allowed);
    }

    struct CmpExtBufById
    {
        mfxU32 id;

        CmpExtBufById(mfxU32 _id)
            : id(_id)
        { };

        bool operator () (mfxExtBuffer* b)
        {
            return  (b && b->BufferId == id);
        };
    };

    static std::string Fourcc2Str(mfxU32 fourcc)
    {
        std::string s;
        for (size_t i = 0; i < 4; i++)
        {
            s.push_back(*(i + (char*)&fourcc));
        }
        return s;
    }

    std::vector<mfxExtBuffer*> m_ext_buf;
};

using MfxVideoParamsWrapper = ExtBufHolder<mfxVideoParam>;
using mfxEncodeCtrlWrap     = ExtBufHolder<mfxEncodeCtrl>;
using mfxInitParamlWrap     = ExtBufHolder<mfxInitParam>;
using mfxFrameSurfaceWrap   = ExtBufHolder<mfxFrameSurface1>;

class mfxBitstreamWrapper : public ExtBufHolder<mfxBitstream>
{
    typedef ExtBufHolder<mfxBitstream> base;
public:
    mfxBitstreamWrapper()
        : base()
    {}

    mfxBitstreamWrapper(mfxU32 n_bytes)
        : base()
    {
        Extend(n_bytes);
    }

    mfxBitstreamWrapper(const mfxBitstreamWrapper & bs_wrapper)
        : base(bs_wrapper)
        , m_data(bs_wrapper.m_data)
    {
        Data = m_data.data();
    }

    mfxBitstreamWrapper& operator=(mfxBitstreamWrapper const& bs_wrapper)
    {
        mfxBitstreamWrapper tmp(bs_wrapper);

        *this = std::move(tmp);

        return *this;
    }

    mfxBitstreamWrapper(mfxBitstreamWrapper && bs_wrapper)             = default;
    mfxBitstreamWrapper & operator= (mfxBitstreamWrapper&& bs_wrapper) = default;
    ~mfxBitstreamWrapper()                                             = default;

    void Extend(mfxU32 n_bytes)
    {
        if (MaxLength >= n_bytes)
            return;

        m_data.resize(n_bytes);

        Data      = m_data.data();
        MaxLength = n_bytes;
    }
private:
    std::vector<mfxU8> m_data;
};

class CSmplYUVReader
{
public :
    typedef std::list<msdk_string>::iterator ls_iterator;
    CSmplYUVReader();
    virtual ~CSmplYUVReader();

    virtual void Close();
    virtual mfxStatus Init(std::list<msdk_string> inputs, mfxU32 ColorFormat, bool shouldShiftP010=false);
    virtual mfxStatus SkipNframesFromBeginning(mfxU16 w, mfxU16 h, mfxU32 viewId, mfxU32 nframes);
    virtual mfxStatus LoadNextFrame(mfxFrameSurface1* pSurface);
    virtual void Reset();
    mfxU32 m_ColorFormat; // color format of input YUV data, YUV420 or NV12

protected:

    std::vector<FILE*> m_files;

    bool shouldShift10BitsHigh;
    bool m_bInited;
};

class CSmplBitstreamWriter
{
public :

    CSmplBitstreamWriter();
    virtual ~CSmplBitstreamWriter();

    virtual mfxStatus Init(const msdk_char *strFileName);
    virtual mfxStatus WriteNextFrame(mfxBitstream *pMfxBitstream, bool isPrint = true);
    virtual mfxStatus Reset();
    virtual void Close();
    mfxU32 m_nProcessedFramesNum;

protected:
    FILE*       m_fSource;
    bool        m_bInited;
    msdk_string m_sFile;
};

class CSmplYUVWriter
{
public :

    CSmplYUVWriter();
    virtual ~CSmplYUVWriter();

    virtual void      Close();
    virtual mfxStatus Init(const msdk_char *strFileName, const mfxU32 numViews);
    virtual mfxStatus Reset();
    virtual mfxStatus WriteNextFrame(mfxFrameSurface1 *pSurface);
    virtual mfxStatus WriteNextFrameI420(mfxFrameSurface1 *pSurface);

    void SetMultiView() { m_bIsMultiView = true; }

protected:
    FILE         *m_fDest, **m_fDestMVC;
    bool         m_bInited, m_bIsMultiView;
    mfxU32       m_numCreatedFiles;
    msdk_string  m_sFile;
    mfxU32       m_nViews;
};

class CSmplBitstreamReader
{
public :

    CSmplBitstreamReader();
    virtual ~CSmplBitstreamReader();

    //resets position to file begin
    virtual void      Reset();
    virtual void      Close();
    virtual mfxStatus Init(const msdk_char *strFileName);
    virtual mfxStatus ReadNextFrame(mfxBitstream *pBS);

protected:
    FILE*     m_fSource;
    bool      m_bInited;
};

class CH264FrameReader : public CSmplBitstreamReader
{
public:
    CH264FrameReader();
    virtual ~CH264FrameReader();

    /** Free resources.*/
    virtual void      Close();
    virtual mfxStatus Init(const msdk_char *strFileName);
    virtual mfxStatus ReadNextFrame(mfxBitstream *pBS);

private:
    mfxBitstream *m_processedBS;
    // input bit stream
    mfxBitstreamWrapper m_originalBS;

    mfxStatus PrepareNextFrame(mfxBitstream *in, mfxBitstream **out);

    // is stream ended
    bool m_isEndOfStream;

    std::unique_ptr<AbstractSplitter> m_pNALSplitter;
    FrameSplitterInfo *m_frame;
    mfxU8 *m_plainBuffer;
    mfxU32 m_plainBufferSize;
    mfxBitstream m_outBS;
};

//provides output bistream with at least 1 frame, reports about error
class CJPEGFrameReader : public CSmplBitstreamReader
{
    enum JPEGMarker
    {
        SOI=0xD8FF,
        EOI=0xD9FF
    };
public:
    virtual mfxStatus ReadNextFrame(mfxBitstream *pBS);
protected:
    mfxU32 FindMarker(mfxBitstream *pBS,mfxU32 startOffset,JPEGMarker marker);
};

//appends output bistream with exactly 1 frame, reports about error
class CIVFFrameReader : public CSmplBitstreamReader
{
public:
    CIVFFrameReader();
    virtual void      Reset();
    virtual mfxStatus Init(const msdk_char *strFileName);
    virtual mfxStatus ReadNextFrame(mfxBitstream *pBS);

protected:

      /*bytes 0-3    signature: 'DKIF'
    bytes 4-5    version (should be 0)
    bytes 6-7    length of header in bytes
    bytes 8-11   codec FourCC (e.g., 'VP80')
    bytes 12-13  width in pixels
    bytes 14-15  height in pixels
    bytes 16-19  frame rate
    bytes 20-23  time scale
    bytes 24-27  number of frames in file
    bytes 28-31  unused*/

    struct DKIFHrd
    {
        mfxU32 dkif;
        mfxU16 version;
        mfxU16 header_len;
        mfxU32 codec_FourCC;
        mfxU16 width;
        mfxU16 height;
        mfxU32 frame_rate;
        mfxU32 time_scale;
        mfxU32 num_frames;
        mfxU32 unused;
    }m_hdr;
    mfxStatus ReadHeader();
};

// writes bitstream to duplicate-file & supports joining
// (for ViewOutput encoder mode)
class CSmplBitstreamDuplicateWriter : public CSmplBitstreamWriter
{
public:
    CSmplBitstreamDuplicateWriter();

    virtual mfxStatus InitDuplicate(const msdk_char *strFileName);
    virtual mfxStatus JoinDuplicate(CSmplBitstreamDuplicateWriter *pJoinee);
    virtual mfxStatus WriteNextFrame(mfxBitstream *pMfxBitstream, bool isPrint = true);
    virtual void Close();
protected:
    FILE*     m_fSourceDuplicate;
    bool      m_bJoined;
};

//timeinterval calculation helper

template <int tag = 0>
class CTimeInterval : private no_copy
{
    static double g_Freq;
    double       &m_start;
    double        m_own;//reference to this if external counter not required
    //since QPC functions are quite slow it makes sense to optionally enable them
    bool         m_bEnable;
    msdk_tick    m_StartTick;

public:
    CTimeInterval(double &dRef , bool bEnable = true)
        : m_start(dRef)
        , m_bEnable(bEnable)
        , m_StartTick(0)
    {
        if (!m_bEnable)
            return;
        Initialize();
    }
    CTimeInterval(bool bEnable = true)
        : m_start(m_own)
        , m_own()
        , m_bEnable(bEnable)
        , m_StartTick(0)
    {
        if (!m_bEnable)
            return;
        Initialize();
    }

    //updates external value with current time
    double Commit()
    {
        if (!m_bEnable)
            return 0.0;

        if (0.0 != g_Freq)
        {
            m_start = MSDK_GET_TIME(msdk_time_get_tick(), m_StartTick, g_Freq);
        }
        return m_start;
    }
    //last comitted value
    double Last()
    {
        return m_start;
    }
    ~CTimeInterval()
    {
        Commit();
    }
private:
    void Initialize()
    {
        if (0.0 == g_Freq)
        {
            g_Freq = (double)msdk_time_get_frequency();
        }
        m_StartTick = msdk_time_get_tick();
    }
};

template <int tag>double CTimeInterval<tag>::g_Freq = 0.0f;

/** Helper class to measure execution time of some code. Use this class
 * if you need manual measurements.
 *
 * Usage example:
 * {
 *   CTimer timer;
 *   msdk_tick summary_tick;
 *
 *   timer.Start()
 *   function_to_measure();
 *   summary_tick = timer.GetDelta();
 *   printf("Elapsed time 1: %f\n", timer.GetTime());
 *   ...
 *   if (condition) timer.Start();
     function_to_measure();
 *   if (condition) {
 *     summary_tick += timer.GetDelta();
 *     printf("Elapsed time 2: %f\n", timer.GetTime();
 *   }
 *   printf("Overall time: %f\n", CTimer::ConvertToSeconds(summary_tick);
 * }
 */
class CTimer
{
public:
    CTimer():
        start(0)
    {
    }
    static msdk_tick GetFrequency()
    {
        if (!frequency) frequency = msdk_time_get_frequency();
        return frequency;
    }
    static mfxF64 ConvertToSeconds(msdk_tick elapsed)
    {
        return MSDK_GET_TIME(elapsed, 0, GetFrequency());
    }

    inline void Start()
    {
        start = msdk_time_get_tick();
    }
    inline msdk_tick GetDelta()
    {
        return msdk_time_get_tick() - start;
    }
    inline mfxF64 GetTime()
    {
        return MSDK_GET_TIME(msdk_time_get_tick(), start, GetFrequency());
    }

protected:
    static msdk_tick frequency;
    msdk_tick start;
private:
    CTimer(const CTimer&);
    void operator=(const CTimer&);
};

/** Helper class to measure overall execution time of some code. Use this
 * class if you want to measure execution time of the repeatedly executed
 * code.
 *
 * Usage example 1:
 *
 * msdk_tick summary_tick = 0;
 *
 * void function() {
 *
 * {
 *   CAutoTimer timer(&summary_tick);
 *   ...
 * }
 *     ...
 * int main() {
 *   for (;condition;) {
 *     function();
 *   }
 *   printf("Elapsed time: %f\n", CTimer::ConvertToSeconds(summary_tick);
 *   return 0;
 * }
 *
 * Usage example 2:
 * {
 *   msdk_tick summary_tick = 0;
 *
 *   {
 *     CAutoTimer timer(&summary_tick);
 *
 *     for (;condition;) {
 *       ...
 *       {
 *         function_to_measure();
 *         timer.Sync();
 *         printf("Progress: %f\n", CTimer::ConvertToSeconds(summary_tick);
 *       }
 *       ...
 *     }
 *   }
 *   printf("Elapsed time: %f\n", CTimer::ConvertToSeconds(summary_tick);
 * }
 *
 */
class CAutoTimer
{
public:
    CAutoTimer(msdk_tick& _elapsed):
        elapsed(_elapsed),
        start(0)
    {
        elapsed = _elapsed;
        start = msdk_time_get_tick();
    }
    ~CAutoTimer()
    {
        elapsed += msdk_time_get_tick() - start;
    }
    msdk_tick Sync()
    {
        msdk_tick cur = msdk_time_get_tick();
        elapsed += cur - start;
        start = cur;
        return elapsed;
    }
protected:
    msdk_tick& elapsed;
    msdk_tick start;
private:
    CAutoTimer(const CAutoTimer&);
    void operator=(const CAutoTimer&);
};

mfxStatus ConvertFrameRate(mfxF64 dFrameRate, mfxU32* pnFrameRateExtN, mfxU32* pnFrameRateExtD);
mfxF64 CalculateFrameRate(mfxU32 nFrameRateExtN, mfxU32 nFrameRateExtD);

template <class T>
mfxU16 GetFreeSurfaceIndex(T* pSurfacesPool, mfxU16 nPoolSize)
{
    constexpr mfxU16 MSDK_INVALID_SURF_IDX = 0xffff;

    if (pSurfacesPool)
    {
        for (mfxU16 i = 0; i < nPoolSize; i++)
        {
            if (0 == pSurfacesPool[i].Data.Locked)
            {
                return i;
            }
        }
    }
    return MSDK_INVALID_SURF_IDX;
}

mfxU16 GetFreeSurface(mfxFrameSurface1* pSurfacesPool, mfxU16 nPoolSize);
void FreeSurfacePool(mfxFrameSurface1* pSurfacesPool, mfxU16 nPoolSize);

mfxU16 CalculateDefaultBitrate(mfxU32 nCodecId, mfxU32 nTargetUsage, mfxU32 nWidth, mfxU32 nHeight, mfxF64 dFrameRate);

//serialization fnc set
std::basic_string<msdk_char> CodecIdToStr(mfxU32 nFourCC);
mfxU16 StrToTargetUsage(msdk_string strInput);
const msdk_char* TargetUsageToStr(mfxU16 tu);
const msdk_char* ColorFormatToStr(mfxU32 format);
const msdk_char* MfxStatusToStr(mfxStatus sts);

// sets bitstream->PicStruct parsing first APP0 marker in bitstream
mfxStatus MJPEG_AVI_ParsePicStruct(mfxBitstream *bitstream);

// For MVC encoding/decoding purposes
std::basic_string<msdk_char> FormMVCFileName(const msdk_char *strFileName, const mfxU32 numView);

//piecewise linear function for bitrate approximation
class PartiallyLinearFNC
{
    mfxF64 *m_pX;
    mfxF64 *m_pY;
    mfxU32  m_nPoints;
    mfxU32  m_nAllocated;

public:
    PartiallyLinearFNC();
    ~PartiallyLinearFNC();

    void AddPair(mfxF64 x, mfxF64 y);
    mfxF64 at(mfxF64);
private:
    DISALLOW_COPY_AND_ASSIGN(PartiallyLinearFNC);
};

// function for getting a pointer to a specific external buffer from the array
mfxExtBuffer* GetExtBuffer(mfxExtBuffer** ebuffers, mfxU32 nbuffers, mfxU32 BufferId);

// returns false if buf length is insufficient, otherwise
// skips step bytes in buf with specified length and returns true
template <typename Buf_t, typename Length_t>
bool skip(const Buf_t *&buf, Length_t &length, Length_t step)
{
    if (length < step)
        return false;

    buf    += step;
    length -= step;

    return true;
}

//do not link MediaSDK dispatched if class not used
struct MSDKAdapter {
    // returns the number of adapter associated with MSDK session, 0 for SW session
    static mfxU32 GetNumber(mfxSession session, mfxIMPL implVia = 0) {
        mfxU32 adapterNum = 0; // default
        mfxIMPL impl = MFX_IMPL_SOFTWARE; // default in case no HW IMPL is found

        // we don't care for error codes in further code; if something goes wrong we fall back to the default adapter
        if (session)
        {
            MFXQueryIMPL(session, &impl);
        }
        else
        {
            // an auxiliary session, internal for this function
            mfxSession auxSession;
            memset(&auxSession, 0, sizeof(auxSession));

            mfxVersion ver = { {1, 1 }}; // minimum API version which supports multiple devices
            MFXInit(MFX_IMPL_HARDWARE_ANY | implVia, &ver, &auxSession);
            MFXQueryIMPL(auxSession, &impl);
            MFXClose(auxSession);
        }

        // extract the base implementation type
        mfxIMPL baseImpl = MFX_IMPL_BASETYPE(impl);

        const struct
        {
            // actual implementation
            mfxIMPL impl;
            // adapter's number
            mfxU32 adapterID;

        } implTypes[] = {
            {MFX_IMPL_HARDWARE, 0},
            {MFX_IMPL_SOFTWARE, 0},
            {MFX_IMPL_HARDWARE2, 1},
            {MFX_IMPL_HARDWARE3, 2},
            {MFX_IMPL_HARDWARE4, 3}
        };


        // get corresponding adapter number
        for (mfxU8 i = 0; i < sizeof(implTypes)/sizeof(*implTypes); i++)
        {
            if (implTypes[i].impl == baseImpl)
            {
                adapterNum = implTypes[i].adapterID;
                break;
            }
        }

        return adapterNum;
    }
};

struct APIChangeFeatures {
    bool JpegDecode;
    bool JpegEncode;
    bool MVCDecode;
    bool MVCEncode;
    bool IntraRefresh;
    bool LowLatency;
    bool ViewOutput;
    bool LookAheadBRC;
    bool AudioDecode;
    bool SupportCodecPluginAPI;
};

inline
mfxU32 MakeVersion(mfxU16 major, mfxU16 minor)
{
    return major * 1000 + minor;
}

mfxVersion getMinimalRequiredVersion(const APIChangeFeatures &features);

enum msdkAPIFeature {
    MSDK_FEATURE_NONE,
    MSDK_FEATURE_MVC,
    MSDK_FEATURE_JPEG_DECODE,
    MSDK_FEATURE_LOW_LATENCY,
    MSDK_FEATURE_MVC_VIEWOUTPUT,
    MSDK_FEATURE_JPEG_ENCODE,
    MSDK_FEATURE_LOOK_AHEAD,
    MSDK_FEATURE_PLUGIN_API
};

/* Returns true if feature is supported in the given API version */
bool CheckVersion(mfxVersion* version, msdkAPIFeature feature);

void ConfigureAspectRatioConversion(mfxInfoVPP* pVppInfo);

void SEICalcSizeType(std::vector<mfxU8>& data, mfxU16 type, mfxU32 size);

mfxU8 Char2Hex(msdk_char ch);

enum MsdkTraceLevel {
    MSDK_TRACE_LEVEL_SILENT = -1,
    MSDK_TRACE_LEVEL_CRITICAL = 0,
    MSDK_TRACE_LEVEL_ERROR = 1,
    MSDK_TRACE_LEVEL_WARNING = 2,
    MSDK_TRACE_LEVEL_INFO = 3,
    MSDK_TRACE_LEVEL_DEBUG = 4,
};

msdk_string NoFullPath(const msdk_string &);
int  msdk_trace_get_level();
void msdk_trace_set_level(int);
bool msdk_trace_is_printable(int);

msdk_ostream & operator <<(msdk_ostream & os, MsdkTraceLevel tt);

template<typename T>
    mfxStatus msdk_opt_read(const msdk_char* string, T& value);

template<size_t S>
    mfxStatus msdk_opt_read(const msdk_char* string, msdk_char (&value)[S])
    {
        if (!S)
        {
            return MFX_ERR_UNKNOWN;
        }
        value[0]=0;
    #if defined(_WIN32) || defined(_WIN64)
        value[S - 1] = 0;
        return (0 == _tcsncpy_s(value, string,S-1))? MFX_ERR_NONE: MFX_ERR_UNKNOWN;
    #else
        if (strlen(string) < S) {
            strncpy(value, string, S-1);
            value[S - 1] = 0;
            return MFX_ERR_NONE;
        }
        return MFX_ERR_UNKNOWN;
    #endif
    }

template<typename T>
    inline mfxStatus msdk_opt_read(const msdk_string& string, T& value)
    {
        return msdk_opt_read(string.c_str(), value);
    }

mfxStatus StrFormatToCodecFormatFourCC(msdk_char* strInput, mfxU32 &codecFormat);
msdk_string StatusToString(mfxStatus sts);
mfxI32 getMonitorType(msdk_char* str);

void WaitForDeviceToBecomeFree(MFXVideoSession& session, mfxSyncPoint& syncPoint, mfxStatus& currentStatus);

mfxU16 FourCCToChroma(mfxU32 fourCC);

class FPSLimiter
{
public:
    FPSLimiter()  = default;
    ~FPSLimiter() = default;
    void Reset(mfxU32 fps)
    {
        m_delayTicks = fps ? msdk_time_get_frequency() / fps : 0;
    }
    void Work()
    {
        msdk_tick current_tick = msdk_time_get_tick();
        while( m_delayTicks && (m_startTick + m_delayTicks > current_tick) )
        {
            msdk_tick left_tick = m_startTick + m_delayTicks - current_tick;
            uint32_t sleepTime = (uint32_t)(left_tick * 1000 / msdk_time_get_frequency());
            MSDK_SLEEP(sleepTime);
            current_tick = msdk_time_get_tick();
        };
        m_startTick = msdk_time_get_tick();
    }

protected:
    msdk_tick               m_startTick  = 0;
    msdk_tick               m_delayTicks = 0;
};

#endif //__SAMPLE_UTILS_H__
