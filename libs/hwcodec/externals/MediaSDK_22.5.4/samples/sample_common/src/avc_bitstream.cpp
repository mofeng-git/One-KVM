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


#include "avc_bitstream.h"
#include "sample_defs.h"

namespace ProtectedLibrary
{

mfxStatus DecodeExpGolombOne(mfxU32 **ppBitStream, mfxI32 *pBitOffset,
                                                      mfxI32 *pDst,
                                                      mfxI32 isSigned);

enum
{
    SCLFLAT16     = 0,
    SCLDEFAULT    = 1,
    SCLREDEFINED  = 2
};

const mfxU32 bits_data[] =
{
    (((mfxU32)0x01 << (0)) - 1),
    (((mfxU32)0x01 << (1)) - 1),
    (((mfxU32)0x01 << (2)) - 1),
    (((mfxU32)0x01 << (3)) - 1),
    (((mfxU32)0x01 << (4)) - 1),
    (((mfxU32)0x01 << (5)) - 1),
    (((mfxU32)0x01 << (6)) - 1),
    (((mfxU32)0x01 << (7)) - 1),
    (((mfxU32)0x01 << (8)) - 1),
    (((mfxU32)0x01 << (9)) - 1),
    (((mfxU32)0x01 << (10)) - 1),
    (((mfxU32)0x01 << (11)) - 1),
    (((mfxU32)0x01 << (12)) - 1),
    (((mfxU32)0x01 << (13)) - 1),
    (((mfxU32)0x01 << (14)) - 1),
    (((mfxU32)0x01 << (15)) - 1),
    (((mfxU32)0x01 << (16)) - 1),
    (((mfxU32)0x01 << (17)) - 1),
    (((mfxU32)0x01 << (18)) - 1),
    (((mfxU32)0x01 << (19)) - 1),
    (((mfxU32)0x01 << (20)) - 1),
    (((mfxU32)0x01 << (21)) - 1),
    (((mfxU32)0x01 << (22)) - 1),
    (((mfxU32)0x01 << (23)) - 1),
    (((mfxU32)0x01 << (24)) - 1),
    (((mfxU32)0x01 << (25)) - 1),
    (((mfxU32)0x01 << (26)) - 1),
    (((mfxU32)0x01 << (27)) - 1),
    (((mfxU32)0x01 << (28)) - 1),
    (((mfxU32)0x01 << (29)) - 1),
    (((mfxU32)0x01 << (30)) - 1),
    (((mfxU32)0x01 << (31)) - 1),
    ((mfxU32)0xFFFFFFFF),
};

const mfxU8 default_intra_scaling_list4x4[16]=
{
     6, 13, 20, 28, 13, 20, 28, 32, 20, 28, 32, 37, 28, 32, 37, 42
};
const mfxU8 default_inter_scaling_list4x4[16]=
{
    10, 14, 20, 24, 14, 20, 24, 27, 20, 24, 27, 30, 24, 27, 30, 34
};

const mfxU8 default_intra_scaling_list8x8[64]=
{
     6, 10, 13, 16, 18, 23, 25, 27, 10, 11, 16, 18, 23, 25, 27, 29,
    13, 16, 18, 23, 25, 27, 29, 31, 16, 18, 23, 25, 27, 29, 31, 33,
    18, 23, 25, 27, 29, 31, 33, 36, 23, 25, 27, 29, 31, 33, 36, 38,
    25, 27, 29, 31, 33, 36, 38, 40, 27, 29, 31, 33, 36, 38, 40, 42
};
const mfxU8 default_inter_scaling_list8x8[64]=
{
     9, 13, 15, 17, 19, 21, 22, 24, 13, 13, 17, 19, 21, 22, 24, 25,
    15, 17, 19, 21, 22, 24, 25, 27, 17, 19, 21, 22, 24, 25, 27, 28,
    19, 21, 22, 24, 25, 27, 28, 30, 21, 22, 24, 25, 27, 28, 30, 32,
    22, 24, 25, 27, 28, 30, 32, 33, 24, 25, 27, 28, 30, 32, 33, 35
};

const mfxI32 pre_norm_adjust_index4x4[16] =
{// 0 1 2 3
    0,2,0,2,//0
    2,1,2,1,//1
    0,2,0,2,//2
    2,1,2,1 //3
};

const mfxI32 pre_norm_adjust4x4[6][3] =
{
    {10,16,13},
    {11,18,14},
    {13,20,16},
    {14,23,18},
    {16,25,20},
    {18,29,23}
};

const mfxI32 pre_norm_adjust8x8[6][6] =
{
    {20, 18, 32, 19, 25, 24},
    {22, 19, 35, 21, 28, 26},
    {26, 23, 42, 24, 33, 31},
    {28, 25, 45, 26, 35, 33},
    {32, 28, 51, 30, 40, 38},
    {36, 32, 58, 34, 46, 43}
};
const mfxI32 pre_norm_adjust_index8x8[64] =
{// 0 1 2 3 4 5 6 7
    0,3,4,3,0,3,4,3,//0
    3,1,5,1,3,1,5,1,//1
    4,5,2,5,4,5,2,5,//2
    3,1,5,1,3,1,5,1,//3
    0,3,4,3,0,3,4,3,//4
    3,1,5,1,3,1,5,1,//5
    4,5,2,5,4,5,2,5,//6
    3,1,5,1,3,1,5,1 //7
};

const mfxI32 mp_scan4x4[2][16] =
{
    {
        0,  1,  4,  8,
        5,  2,  3,  6,
        9,  12, 13, 10,
        7,  11, 14, 15
    },
    {
        0,  4,  1,  8,
        12, 5,  9,  13,
        2,  6,  10, 14,
        3,  7,  11, 15
    }
};

const mfxI32 hp_scan8x8[2][64] =
{
    //8x8 zigzag scan
    {
        0, 1, 8,16, 9, 2, 3,10,
        17,24,32,25,18,11, 4, 5,
        12,19,26,33,40,48,41,34,
        27,20,13, 6, 7,14,21,28,
        35,42,49,56,57,50,43,36,
        29,22,15,23,30,37,44,51,
        58,59,52,45,38,31,39,46,
        53,60,61,54,47,55,62,63
    },
    //8x8 field scan
    {
        0, 8,16, 1, 9,24,32,17,
        2,25,40,48,56,33,10, 3,
        18,41,49,57,26,11, 4,19,
        34,42,50,58,27,12, 5,20,
        35,43,51,59,28,13, 6,21,
        36,44,52,60,29,14,22,37,
        45,53,61,30, 7,15,38,46,
        54,62,23,31,39,47,55,63
    }
};

#define avcSkipNBits(current_data, offset, nbits) \
{ \
    /* check error(s) */ \
    SAMPLE_ASSERT((nbits) > 0 && (nbits) <= 32); \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
    /* decrease number of available bits */ \
    offset -= (nbits); \
    /* normalize bitstream pointer */ \
    if (0 > offset) \
    { \
        offset += 32; \
        current_data++; \
    } \
    /* check error(s) again */ \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
 }

#define avcGetBits8( current_data, offset, data) \
    _avcGetBits(current_data, offset, 8, data);


#define avcUngetNBits(current_data, offset, nbits) \
{ \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
 \
    offset += (nbits); \
    if (offset > 31) \
    { \
        offset -= 32; \
        current_data--; \
    } \
 \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
}

#define avcUngetBits32(current_data, offset) \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
    current_data--;

#define avcAlignBSPointerRight(current_data, offset) \
{ \
    if ((offset & 0x07) != 0x07) \
    { \
        offset = (offset | 0x07) - 8; \
        if (offset == -1) \
        { \
            offset = 31; \
            current_data++; \
        } \
    } \
}

#define avcNextBits(current_data, bp, nbits, data) \
{ \
    mfxU32 x; \
 \
    SAMPLE_ASSERT((nbits) > 0 && (nbits) <= 32); \
    SAMPLE_ASSERT(nbits >= 0 && nbits <= 31); \
 \
    mfxI32 offset = bp - (nbits); \
 \
    if (offset >= 0) \
    { \
        x = current_data[0] >> (offset + 1); \
    } \
    else \
    { \
        offset += 32; \
 \
        x = current_data[1] >> (offset); \
        x >>= 1; \
        x += current_data[0] << (31 - offset); \
    } \
 \
    SAMPLE_ASSERT(offset >= 0 && offset <= 31); \
 \
    (data) = x & bits_data[nbits]; \
}

inline void FillFlatScalingList4x4(AVCScalingList4x4 *scl)
{
    for (mfxI32 i=0;i<16;i++)
        scl->ScalingListCoeffs[i] = 16;
}

inline void FillFlatScalingList8x8(AVCScalingList8x8 *scl)
{
    for (mfxI32 i=0;i<64;i++)
        scl->ScalingListCoeffs[i] = 16;
}

inline void FillScalingList4x4(AVCScalingList4x4 *scl_dst,mfxU8 *coefs_src)
{
    for (mfxI32 i=0;i<16;i++)
        scl_dst->ScalingListCoeffs[i] = coefs_src[i];
}

inline void FillScalingList8x8(AVCScalingList8x8 *scl_dst,mfxU8 *coefs_src)
{
    for (mfxI32 i=0;i<64;i++)
        scl_dst->ScalingListCoeffs[i] = coefs_src[i];
}

AVCBaseBitstream::AVCBaseBitstream()
{
    Reset(0, 0);
}

AVCBaseBitstream::AVCBaseBitstream(mfxU8 * const pb, const mfxU32 maxsize)
{
    Reset(pb, maxsize);
}

AVCBaseBitstream::~AVCBaseBitstream()
{
}

void AVCBaseBitstream::Reset(mfxU8 * const pb, const mfxU32 maxsize)
{
    m_pbs       = (mfxU32*)pb;
    m_pbsBase   = (mfxU32*)pb;
    m_bitOffset = 31;
    m_maxBsSize    = maxsize;

} // void Reset(mfxU8 * const pb, const mfxU32 maxsize)

void AVCBaseBitstream::Reset(mfxU8 * const pb, mfxI32 offset, const mfxU32 maxsize)
{
    m_pbs       = (mfxU32*)pb;
    m_pbsBase   = (mfxU32*)pb;
    m_bitOffset = offset;
    m_maxBsSize = maxsize;

} // void Reset(mfxU8 * const pb, mfxI32 offset, const mfxU32 maxsize)

mfxStatus AVCBaseBitstream::GetNALUnitType( NAL_Unit_Type &uNALUnitType,mfxU8 &uNALStorageIDC)
{
    mfxU32 code;
    avcGetBits8(m_pbs, m_bitOffset, code);

    uNALStorageIDC = (mfxU8)((code & NAL_STORAGE_IDC_BITS)>>5);
    uNALUnitType = (NAL_Unit_Type)(code & NAL_UNITTYPE_BITS);
    return MFX_ERR_NONE;
}    // GetNALUnitType

mfxI32 AVCBaseBitstream::GetVLCElement(bool bIsSigned)
{
    mfxI32 sval = 0;

    mfxStatus ippRes = DecodeExpGolombOne(&m_pbs, &m_bitOffset, &sval, bIsSigned);

    if (ippRes < MFX_ERR_NONE)
        throw AVC_exception(MFX_ERR_UNDEFINED_BEHAVIOR);

    return sval;
}

void AVCBaseBitstream::AlignPointerRight(void)
{
    avcAlignBSPointerRight(m_pbs, m_bitOffset);

} // void AVCBitstream::AlignPointerRight(void)

bool AVCBaseBitstream::More_RBSP_Data()
{
    mfxI32 code, tmp;
    mfxU32* ptr_state = m_pbs;
    mfxI32  bit_state = m_bitOffset;

    SAMPLE_ASSERT(m_bitOffset >= 0 && m_bitOffset <= 31);

    mfxI32 remaining_bytes = (mfxI32)BytesLeft();

    if (remaining_bytes <= 0)
        return false;

    // get top bit, it can be "rbsp stop" bit
    avcGetNBits(m_pbs, m_bitOffset, 1, code);

    // get remain bits, which is less then byte
    tmp = (m_bitOffset + 1) % 8;

    if(tmp)
    {
        avcGetNBits(m_pbs, m_bitOffset, tmp, code);
        if ((code << (8 - tmp)) & 0x7f)    // most sig bit could be rbsp stop bit
        {
            m_pbs = ptr_state;
            m_bitOffset = bit_state;
            // there are more data
            return true;
        }
    }

    remaining_bytes = (mfxI32)BytesLeft();

    // run through remain bytes
    while (0 < remaining_bytes)
    {
        avcGetBits8(m_pbs, m_bitOffset, code);

        if (code)
        {
            m_pbs = ptr_state;
            m_bitOffset = bit_state;
            // there are more data
            return true;
        }

        remaining_bytes -= 1;
    }

    return false;
}

AVCHeadersBitstream::AVCHeadersBitstream()
    : AVCBaseBitstream()
{
}

AVCHeadersBitstream::AVCHeadersBitstream(mfxU8 * const pb, const mfxU32 maxsize)
    : AVCBaseBitstream(pb, maxsize)
{
}

// ---------------------------------------------------------------------------
//  AVCBitstream::GetSequenceParamSet()
//    Read sequence parameter set data from bitstream.
// ---------------------------------------------------------------------------
mfxStatus AVCHeadersBitstream::GetSequenceParamSet(AVCSeqParamSet *sps)
{
    // Not all members of the seq param set structure are contained in all
    // seq param sets. So start by init all to zero.
    mfxStatus ps = MFX_ERR_NONE;
    sps->Reset();

    // profile
    sps->profile_idc = (mfxU8)GetBits(8);

    switch (sps->profile_idc)
    {
    case AVC_PROFILE_BASELINE:
    case AVC_PROFILE_MAIN:
    case AVC_PROFILE_SCALABLE_BASELINE:
    case AVC_PROFILE_SCALABLE_HIGH:
    case AVC_PROFILE_EXTENDED:
    case AVC_PROFILE_HIGH:
    case AVC_PROFILE_HIGH10:
    case AVC_PROFILE_MULTIVIEW_HIGH:
    case AVC_PROFILE_HIGH422:
    case AVC_PROFILE_STEREO_HIGH:
    case AVC_PROFILE_HIGH444:
    case AVC_PROFILE_ADVANCED444_INTRA:
    case AVC_PROFILE_ADVANCED444:
        break;
    default:
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    sps->constrained_set0_flag = (mfxU8)Get1Bit();

    sps->constrained_set1_flag = (mfxU8)Get1Bit();

    sps->constrained_set2_flag = (mfxU8)Get1Bit();

    sps->constrained_set3_flag = (mfxU8)Get1Bit();

    // skip 4 zero bits
    GetBits(4);

    sps->level_idc = (mfxU8)GetBits(8);

    switch(sps->level_idc)
    {
    case AVC_LEVEL_1:
    case AVC_LEVEL_11:
    case AVC_LEVEL_12:
    case AVC_LEVEL_13:

    case AVC_LEVEL_2:
    case AVC_LEVEL_21:
    case AVC_LEVEL_22:

    case AVC_LEVEL_3:
    case AVC_LEVEL_31:
    case AVC_LEVEL_32:

    case AVC_LEVEL_4:
    case AVC_LEVEL_41:
    case AVC_LEVEL_42:

    case AVC_LEVEL_5:
    case AVC_LEVEL_51:
        break;
    default:
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    // id
    mfxI32 sps_id = GetVLCElement(false);
    if (sps_id > MAX_NUM_SEQ_PARAM_SETS - 1)
    {
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    sps->seq_parameter_set_id = (mfxU8)sps_id;

    // see 7.3.2.1.1 "Sequence parameter set data syntax"
    // chapter of H264 standard for full list of profiles with chrominance
    if ((AVC_PROFILE_SCALABLE_BASELINE == sps->profile_idc) ||
        (AVC_PROFILE_SCALABLE_HIGH == sps->profile_idc) ||
        (AVC_PROFILE_HIGH == sps->profile_idc) ||
        (AVC_PROFILE_HIGH10 == sps->profile_idc) ||
        (AVC_PROFILE_MULTIVIEW_HIGH == sps->profile_idc) ||
        (AVC_PROFILE_HIGH422 == sps->profile_idc) ||
        (AVC_PROFILE_STEREO_HIGH == sps->profile_idc) ||
        (244 == sps->profile_idc) ||
        (44 == sps->profile_idc))
    {
        mfxU32 chroma_format_idc = GetVLCElement(false);

        if (chroma_format_idc > 3)
            return MFX_ERR_UNDEFINED_BEHAVIOR;

        sps->chroma_format_idc = (mfxU8)chroma_format_idc;
        if (sps->chroma_format_idc==3)
        {
            sps->residual_colour_transform_flag = (mfxU8) Get1Bit();
        }

        mfxU32 bit_depth_luma = GetVLCElement(false) + 8;
        mfxU32 bit_depth_chroma = GetVLCElement(false) + 8;

        if (bit_depth_luma > 16 || bit_depth_chroma > 16)
            return MFX_ERR_UNDEFINED_BEHAVIOR;

        sps->bit_depth_luma = (mfxU8)bit_depth_luma;
        sps->bit_depth_chroma = (mfxU8)bit_depth_chroma;

        if (!chroma_format_idc)
            sps->bit_depth_chroma = sps->bit_depth_luma;

        SAMPLE_ASSERT(!sps->residual_colour_transform_flag);
        if (sps->residual_colour_transform_flag == 1)
        {
            return MFX_ERR_UNDEFINED_BEHAVIOR;
        }

        sps->qpprime_y_zero_transform_bypass_flag = (mfxU8)Get1Bit();
        sps->seq_scaling_matrix_present_flag = (mfxU8)Get1Bit();
        if(sps->seq_scaling_matrix_present_flag)
        {
            // 0
            if(Get1Bit())
            {
                GetScalingList4x4(&sps->ScalingLists4x4[0],(mfxU8*)default_intra_scaling_list4x4,&sps->type_of_scaling_list_used[0]);
            }
            else
            {
                FillScalingList4x4(&sps->ScalingLists4x4[0],(mfxU8*) default_intra_scaling_list4x4);
                sps->type_of_scaling_list_used[0] = SCLDEFAULT;
            }
            // 1
            if(Get1Bit())
            {
                GetScalingList4x4(&sps->ScalingLists4x4[1],(mfxU8*) default_intra_scaling_list4x4,&sps->type_of_scaling_list_used[1]);
            }
            else
            {
                FillScalingList4x4(&sps->ScalingLists4x4[1],(mfxU8*) sps->ScalingLists4x4[0].ScalingListCoeffs);
                sps->type_of_scaling_list_used[1] = SCLDEFAULT;
            }
            // 2
            if(Get1Bit())
            {
                GetScalingList4x4(&sps->ScalingLists4x4[2],(mfxU8*) default_intra_scaling_list4x4,&sps->type_of_scaling_list_used[2]);
            }
            else
            {
                FillScalingList4x4(&sps->ScalingLists4x4[2],(mfxU8*) sps->ScalingLists4x4[1].ScalingListCoeffs);
                sps->type_of_scaling_list_used[2] = SCLDEFAULT;
            }
            // 3
            if(Get1Bit())
            {
                GetScalingList4x4(&sps->ScalingLists4x4[3],(mfxU8*)default_inter_scaling_list4x4,&sps->type_of_scaling_list_used[3]);
            }
            else
            {
                FillScalingList4x4(&sps->ScalingLists4x4[3],(mfxU8*) default_inter_scaling_list4x4);
                sps->type_of_scaling_list_used[3] = SCLDEFAULT;
            }
            // 4
            if(Get1Bit())
            {
                GetScalingList4x4(&sps->ScalingLists4x4[4],(mfxU8*) default_inter_scaling_list4x4,&sps->type_of_scaling_list_used[4]);
            }
            else
            {
                FillScalingList4x4(&sps->ScalingLists4x4[4],(mfxU8*) sps->ScalingLists4x4[3].ScalingListCoeffs);
                sps->type_of_scaling_list_used[4] = SCLDEFAULT;
            }
            // 5
            if(Get1Bit())
            {
                GetScalingList4x4(&sps->ScalingLists4x4[5],(mfxU8*) default_inter_scaling_list4x4,&sps->type_of_scaling_list_used[5]);
            }
            else
            {
                FillScalingList4x4(&sps->ScalingLists4x4[5],(mfxU8*) sps->ScalingLists4x4[4].ScalingListCoeffs);
                sps->type_of_scaling_list_used[5] = SCLDEFAULT;
            }

            // 0
            if(Get1Bit())
            {
                GetScalingList8x8(&sps->ScalingLists8x8[0],(mfxU8*)default_intra_scaling_list8x8,&sps->type_of_scaling_list_used[6]);
            }
            else
            {
                FillScalingList8x8(&sps->ScalingLists8x8[0],(mfxU8*) default_intra_scaling_list8x8);
                sps->type_of_scaling_list_used[6] = SCLDEFAULT;
            }
            // 1
            if(Get1Bit())
            {
                GetScalingList8x8(&sps->ScalingLists8x8[1],(mfxU8*) default_inter_scaling_list8x8,&sps->type_of_scaling_list_used[7]);
            }
            else
            {
                FillScalingList8x8(&sps->ScalingLists8x8[1],(mfxU8*) default_inter_scaling_list8x8);
                sps->type_of_scaling_list_used[7] = SCLDEFAULT;
            }

        }
        else
        {
            mfxI32 i;

            for (i = 0; i < 6; i += 1)
            {
                FillFlatScalingList4x4(&sps->ScalingLists4x4[i]);
            }
            for (i = 0; i < 2; i += 1)
            {
                FillFlatScalingList8x8(&sps->ScalingLists8x8[i]);
            }
        }
    }
    else
    {
        sps->chroma_format_idc = 1;
        sps->bit_depth_luma = 8;
        sps->bit_depth_chroma = 8;

        SetDefaultScalingLists(sps);
    }

    // log2 max frame num (bitstream contains value - 4)
    mfxU32 log2_max_frame_num = GetVLCElement(false) + 4;
    sps->log2_max_frame_num = (mfxU8)log2_max_frame_num;

    if (log2_max_frame_num > 16 || log2_max_frame_num < 4)
        return MFX_ERR_UNDEFINED_BEHAVIOR;

    // pic order cnt type (0..2)
    mfxU32 pic_order_cnt_type = GetVLCElement(false);
    sps->pic_order_cnt_type = (mfxU8)pic_order_cnt_type;
    if (pic_order_cnt_type > 2)
    {
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    if (sps->pic_order_cnt_type == 0)
    {
        // log2 max pic order count lsb (bitstream contains value - 4)
        mfxU32 log2_max_pic_order_cnt_lsb = GetVLCElement(false) + 4;
        sps->log2_max_pic_order_cnt_lsb = (mfxU8)log2_max_pic_order_cnt_lsb;

        if (log2_max_pic_order_cnt_lsb > 16 || log2_max_pic_order_cnt_lsb < 4)
            return MFX_ERR_UNDEFINED_BEHAVIOR;

        sps->MaxPicOrderCntLsb = (1 << sps->log2_max_pic_order_cnt_lsb);
    }
    else if (sps->pic_order_cnt_type == 1)
    {
        sps->delta_pic_order_always_zero_flag = (mfxU8)Get1Bit();
        sps->offset_for_non_ref_pic = GetVLCElement(true);
        sps->offset_for_top_to_bottom_field = GetVLCElement(true);
        sps->num_ref_frames_in_pic_order_cnt_cycle = GetVLCElement(false);

        if (sps->num_ref_frames_in_pic_order_cnt_cycle > 255)
            return MFX_ERR_UNDEFINED_BEHAVIOR;

        // get offsets
        for (mfxU32 i = 0; i < sps->num_ref_frames_in_pic_order_cnt_cycle; i++)
        {
            sps->poffset_for_ref_frame[i] = GetVLCElement(true);
        }
    }    // pic order count type 1

    // num ref frames
    sps->num_ref_frames = GetVLCElement(false);
    if (sps->num_ref_frames > 16)
        return MFX_ERR_UNDEFINED_BEHAVIOR;

    sps->gaps_in_frame_num_value_allowed_flag = (mfxU8)Get1Bit();

    // picture width in MBs (bitstream contains value - 1)
    sps->frame_width_in_mbs = GetVLCElement(false) + 1;

    // picture height in MBs (bitstream contains value - 1)
    sps->frame_height_in_mbs = GetVLCElement(false) + 1;

    sps->frame_mbs_only_flag = (mfxU8)Get1Bit();
    sps->frame_height_in_mbs  = (2-sps->frame_mbs_only_flag)*sps->frame_height_in_mbs;
    if (sps->frame_mbs_only_flag == 0)
    {
        sps->mb_adaptive_frame_field_flag = (mfxU8)Get1Bit();
    }
    sps->direct_8x8_inference_flag = (mfxU8)Get1Bit();
    if (sps->frame_mbs_only_flag==0)
    {
        sps->direct_8x8_inference_flag = 1;
    }
    sps->frame_cropping_flag = (mfxU8)Get1Bit();

    if (sps->frame_cropping_flag)
    {
        sps->frame_cropping_rect_left_offset      = GetVLCElement(false);
        sps->frame_cropping_rect_right_offset     = GetVLCElement(false);
        sps->frame_cropping_rect_top_offset       = GetVLCElement(false);
        sps->frame_cropping_rect_bottom_offset    = GetVLCElement(false);
    } // don't need else because we zeroid structure

    sps->vui_parameters_present_flag = (mfxU8)Get1Bit();
    if (sps->vui_parameters_present_flag)
    {
        if (ps == MFX_ERR_NONE)
            ps = GetVUIParam(sps);
    }

    return ps;
}    // GetSequenceParamSet

mfxStatus AVCHeadersBitstream::GetVUIParam(AVCSeqParamSet *sps)
{
    mfxStatus ps=MFX_ERR_NONE;
    sps->aspect_ratio_info_present_flag = (mfxU8) Get1Bit();

    sps->sar_width = 1; // default values
    sps->sar_height = 1;

    if (sps->aspect_ratio_info_present_flag)
    {
        sps->aspect_ratio_idc = (mfxU8) GetBits(8);
        if (sps->aspect_ratio_idc  ==  255) {
            sps->sar_width = (mfxU16) GetBits(16);
            sps->sar_height = (mfxU16) GetBits(16);
        }
    }

    sps->overscan_info_present_flag = (mfxU8) Get1Bit();
    if( sps->overscan_info_present_flag )
        sps->overscan_appropriate_flag = (mfxU8) Get1Bit();
    sps->video_signal_type_present_flag = (mfxU8) Get1Bit();
    if( sps->video_signal_type_present_flag ) {
        sps->video_format = (mfxU8) GetBits(3);
        sps->video_full_range_flag = (mfxU8) Get1Bit();
        sps->colour_description_present_flag = (mfxU8) Get1Bit();
        if( sps->colour_description_present_flag ) {
            sps->colour_primaries = (mfxU8) GetBits(8);
            sps->transfer_characteristics = (mfxU8) GetBits(8);
            sps->matrix_coefficients = (mfxU8) GetBits(8);
        }
    }
    sps->chroma_loc_info_present_flag = (mfxU8) Get1Bit();
    if( sps->chroma_loc_info_present_flag ) {
        sps->chroma_sample_loc_type_top_field = (mfxU8) GetVLCElement(false);
        sps->chroma_sample_loc_type_bottom_field = (mfxU8) GetVLCElement(false);
    }
    sps->timing_info_present_flag = (mfxU8) Get1Bit();

    if (sps->timing_info_present_flag)
    {
        sps->num_units_in_tick = GetBits(32);
        sps->time_scale = GetBits(32);
        sps->fixed_frame_rate_flag = (mfxU8) Get1Bit();

        if (!sps->num_units_in_tick || !sps->time_scale)
            sps->timing_info_present_flag = 0;
    }

    sps->nal_hrd_parameters_present_flag = (mfxU8) Get1Bit();
    if( sps->nal_hrd_parameters_present_flag )
        ps=GetHRDParam(sps);
    sps->vcl_hrd_parameters_present_flag = (mfxU8) Get1Bit();
    if( sps->vcl_hrd_parameters_present_flag )
        ps=GetHRDParam(sps);
    if( sps->nal_hrd_parameters_present_flag  ||  sps->vcl_hrd_parameters_present_flag )
        sps->low_delay_hrd_flag = (mfxU8) Get1Bit();
    sps->pic_struct_present_flag  = (mfxU8) Get1Bit();
    sps->bitstream_restriction_flag = (mfxU8) Get1Bit();
    if( sps->bitstream_restriction_flag ) {
        sps->motion_vectors_over_pic_boundaries_flag = (mfxU8) Get1Bit();
        sps->max_bytes_per_pic_denom = (mfxU8) GetVLCElement(false);
        sps->max_bits_per_mb_denom = (mfxU8) GetVLCElement(false);
        sps->log2_max_mv_length_horizontal = (mfxU8) GetVLCElement(false);
        sps->log2_max_mv_length_vertical = (mfxU8) GetVLCElement(false);
        sps->num_reorder_frames = (mfxU8) GetVLCElement(false);

        mfxI32 value = GetVLCElement(false);
        if (value < (mfxI32)sps->num_ref_frames || value < 0)
        {
            return MFX_ERR_UNDEFINED_BEHAVIOR;
        }

        sps->max_dec_frame_buffering = (mfxU8) GetVLCElement(false);
    }
    return ps;
}

mfxStatus AVCHeadersBitstream::GetHRDParam(AVCSeqParamSet *sps)
{
    mfxStatus ps=MFX_ERR_NONE;
    mfxI32 cpb_cnt = GetVLCElement(false) + 1;

    if (cpb_cnt >= 32)
    {
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    sps->cpb_cnt = (mfxU8)cpb_cnt;

    sps->bit_rate_scale = (mfxU8) GetBits(4);
    sps->cpb_size_scale = (mfxU8) GetBits(4);
    for( mfxI32 idx= 0; idx < sps->cpb_cnt; idx++ ) {
        sps->bit_rate_value[ idx ] = (mfxU32) (GetVLCElement(false)+1);
        sps->cpb_size_value[ idx ] = (mfxU32) ((GetVLCElement(false)+1));
        sps->cbr_flag[ idx ] = (mfxU8) Get1Bit();
    }
    sps->initial_cpb_removal_delay_length = (mfxU8)(GetBits(5)+1);
    sps->cpb_removal_delay_length = (mfxU8)(GetBits(5)+1);
    sps->dpb_output_delay_length = (mfxU8) (GetBits(5)+1);
    sps->time_offset_length = (mfxU8) GetBits(5);
    return ps;

}

// ---------------------------------------------------------------------------
//    Read sequence parameter set extension data from bitstream.
// ---------------------------------------------------------------------------
mfxStatus AVCHeadersBitstream::GetSequenceParamSetExtension(AVCSeqParamSetExtension *sps_ex)
{
    // Not all members of the seq param set structure are contained in all
    // seq param sets. So start by init all to zero.
    mfxStatus ps = MFX_ERR_NONE;
    sps_ex->Reset();

    mfxU32 seq_parameter_set_id = GetVLCElement(false);
    sps_ex->seq_parameter_set_id = (mfxU8)seq_parameter_set_id;
    if (seq_parameter_set_id > MAX_NUM_SEQ_PARAM_SETS-1)
    {
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    mfxU32 aux_format_idc = GetVLCElement(false);
    sps_ex->aux_format_idc = (mfxU8)aux_format_idc;
    if (aux_format_idc > 3)
    {
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    if (sps_ex->aux_format_idc != 1 && sps_ex->aux_format_idc != 2)
        sps_ex->aux_format_idc = 0;

    if (sps_ex->aux_format_idc)
    {
        mfxU32 bit_depth_aux = GetVLCElement(false) + 8;
        sps_ex->bit_depth_aux = (mfxU8)bit_depth_aux;
        if (bit_depth_aux > 12)
        {
            return MFX_ERR_UNDEFINED_BEHAVIOR;
        }

        sps_ex->alpha_incr_flag = (mfxU8)Get1Bit();
        sps_ex->alpha_opaque_value = (mfxU8)GetBits(sps_ex->bit_depth_aux + 1);
        sps_ex->alpha_transparent_value = (mfxU8)GetBits(sps_ex->bit_depth_aux + 1);
    }

    sps_ex->additional_extension_flag = (mfxU8)Get1Bit();

    return ps;
}    // GetSequenceParamSetExtension

mfxStatus AVCHeadersBitstream::GetPictureParamSetPart1(AVCPicParamSet *pps)
{
    // Not all members of the pic param set structure are contained in all
    // pic param sets. So start by init all to zero.
    pps->Reset();

    // id
    mfxU32 pic_parameter_set_id = GetVLCElement(false);
    pps->pic_parameter_set_id = (mfxU16)pic_parameter_set_id;
    if (pic_parameter_set_id > MAX_NUM_PIC_PARAM_SETS-1)
    {
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    // seq param set referred to by this pic param set
    mfxU32 seq_parameter_set_id = GetVLCElement(false);
    pps->seq_parameter_set_id = (mfxU8)seq_parameter_set_id;
    if (seq_parameter_set_id > MAX_NUM_SEQ_PARAM_SETS-1)
    {
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    return MFX_ERR_NONE;
}    // GetPictureParamSetPart1

// Number of bits required to code slice group ID, index is num_slice_groups - 2
static const mfxU8 SGIdBits[7] = {1,2,2,3,3,3,3};

// ---------------------------------------------------------------------------
//    Read picture parameter set data from bitstream.
// ---------------------------------------------------------------------------
mfxStatus AVCHeadersBitstream::GetPictureParamSetPart2(AVCPicParamSet  *pps,
                                              const AVCSeqParamSet *sps)
{
    pps->entropy_coding_mode = (mfxU8)Get1Bit();


    pps->pic_order_present_flag = (mfxU8)Get1Bit();

    // number of slice groups, bitstream has value - 1
    pps->num_slice_groups = GetVLCElement(false) + 1;
    if (pps->num_slice_groups != 1)
    {
        mfxU32 slice_group;
        mfxU32 PicSizeInMapUnits;    // for range checks

        PicSizeInMapUnits = sps->frame_width_in_mbs * sps->frame_height_in_mbs;
            // TBD: needs adjust for fields

        if (pps->num_slice_groups > MAX_NUM_SLICE_GROUPS)
        {
            return MFX_ERR_UNDEFINED_BEHAVIOR;
        }

        mfxU32 slice_group_map_type = GetVLCElement(false);
        pps->SliceGroupInfo.slice_group_map_type = (mfxU8)slice_group_map_type;

        if (slice_group_map_type > 6)
            return MFX_ERR_UNDEFINED_BEHAVIOR;

        // Get additional, map type dependent slice group data
        switch (pps->SliceGroupInfo.slice_group_map_type)
        {
        case 0:
            for (slice_group=0; slice_group<pps->num_slice_groups; slice_group++)
            {
                // run length, bitstream has value - 1
                pps->SliceGroupInfo.run_length[slice_group] = GetVLCElement(false) + 1;

                if (pps->SliceGroupInfo.run_length[slice_group] > PicSizeInMapUnits)
                {
                    return MFX_ERR_UNDEFINED_BEHAVIOR;
                }
            }
            break;
        case 1:
            // no additional info
            break;
        case 2:
            for (slice_group=0; slice_group<(mfxU32)(pps->num_slice_groups-1); slice_group++)
            {
                pps->SliceGroupInfo.t1.top_left[slice_group] = GetVLCElement(false);
                pps->SliceGroupInfo.t1.bottom_right[slice_group] = GetVLCElement(false);

                // check for legal values
                if (pps->SliceGroupInfo.t1.top_left[slice_group] >
                    pps->SliceGroupInfo.t1.bottom_right[slice_group])
                {
                    return MFX_ERR_UNDEFINED_BEHAVIOR;
                }
                if (pps->SliceGroupInfo.t1.bottom_right[slice_group] >= PicSizeInMapUnits)
                {
                    return MFX_ERR_UNDEFINED_BEHAVIOR;
                }
                if ((pps->SliceGroupInfo.t1.top_left[slice_group] %
                    sps->frame_width_in_mbs) >
                    (pps->SliceGroupInfo.t1.bottom_right[slice_group] %
                    sps->frame_width_in_mbs))
                {
                    return MFX_ERR_UNDEFINED_BEHAVIOR;
                }
            }
            break;
        case 3:
        case 4:
        case 5:
            // For map types 3..5, number of slice groups must be 2
            if (pps->num_slice_groups != 2)
            {
                return MFX_ERR_UNDEFINED_BEHAVIOR;
            }
            pps->SliceGroupInfo.t2.slice_group_change_direction_flag = (mfxU8)Get1Bit();
            pps->SliceGroupInfo.t2.slice_group_change_rate = GetVLCElement(false) + 1;
            if (pps->SliceGroupInfo.t2.slice_group_change_rate > PicSizeInMapUnits)
            {
                return MFX_ERR_UNDEFINED_BEHAVIOR;
            }
            break;
        case 6:
            // mapping of slice group to map unit (macroblock if not fields) is
            // per map unit, read from bitstream
            {
                mfxU32 map_unit;
                mfxU32 num_bits;    // number of bits used to code each slice group id

                // number of map units, bitstream has value - 1
                pps->SliceGroupInfo.t3.pic_size_in_map_units = GetVLCElement(false) + 1;
                if (pps->SliceGroupInfo.t3.pic_size_in_map_units != PicSizeInMapUnits)
                {
                    return MFX_ERR_UNDEFINED_BEHAVIOR;
                }

                mfxI32 len = std::max(1u, pps->SliceGroupInfo.t3.pic_size_in_map_units);

                pps->SliceGroupInfo.pSliceGroupIDMap.resize(len);

                // num_bits is Ceil(log2(num_groups))
                num_bits = SGIdBits[pps->num_slice_groups - 2];

                for (map_unit = 0;
                     map_unit < pps->SliceGroupInfo.t3.pic_size_in_map_units;
                     map_unit++)
                {
                    pps->SliceGroupInfo.pSliceGroupIDMap[map_unit] = (mfxU8)GetBits(num_bits);
                    if (pps->SliceGroupInfo.pSliceGroupIDMap[map_unit] >
                        pps->num_slice_groups - 1)
                    {
                        return MFX_ERR_UNDEFINED_BEHAVIOR;
                    }
                }
            }
            break;
        default:
            return MFX_ERR_UNDEFINED_BEHAVIOR;

        }    // switch
    }    // slice group info

    // number of list 0 ref pics used to decode picture, bitstream has value - 1
    pps->num_ref_idx_l0_active = GetVLCElement(false) + 1;

    // number of list 1 ref pics used to decode picture, bitstream has value - 1
    pps->num_ref_idx_l1_active = GetVLCElement(false) + 1;

    if (pps->num_ref_idx_l1_active > MAX_NUM_REF_FRAMES || pps->num_ref_idx_l0_active > MAX_NUM_REF_FRAMES)
        return MFX_ERR_UNDEFINED_BEHAVIOR;

    // weighted pediction
    pps->weighted_pred_flag = (mfxU8)Get1Bit();
    pps->weighted_bipred_idc = (mfxU8)GetBits(2);

    // default slice QP, bitstream has value - 26
    mfxI32 pic_init_qp = GetVLCElement(true) + 26;
    pps->pic_init_qp = (mfxI8)pic_init_qp;

    // default SP/SI slice QP, bitstream has value - 26
    pps->pic_init_qs = (mfxU8)(GetVLCElement(true) + 26);
    pps->chroma_qp_index_offset[0] = (mfxI8)GetVLCElement(true);

    pps->deblocking_filter_variables_present_flag = (mfxU8)Get1Bit();
    pps->constrained_intra_pred_flag = (mfxU8)Get1Bit();
    pps->redundant_pic_cnt_present_flag = (mfxU8)Get1Bit();
    if (More_RBSP_Data())
    {
        pps->transform_8x8_mode_flag = (mfxU8) Get1Bit();
        if(sps->seq_scaling_matrix_present_flag)
        {
            //fall-back set rule B
            if(Get1Bit())
            {
                // 0
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[0],(mfxU8*)default_intra_scaling_list4x4,&pps->type_of_scaling_list_used[0]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[0],(mfxU8*) sps->ScalingLists4x4[0].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[0] = SCLDEFAULT;
                }
                // 1
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[1],(mfxU8*) default_intra_scaling_list4x4,&pps->type_of_scaling_list_used[1]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[1],(mfxU8*) pps->ScalingLists4x4[0].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[1] = SCLDEFAULT;
                }
                // 2
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[2],(mfxU8*) default_intra_scaling_list4x4,&pps->type_of_scaling_list_used[2]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[2],(mfxU8*) pps->ScalingLists4x4[1].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[2] = SCLDEFAULT;
                }
                // 3
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[3],(mfxU8*) default_inter_scaling_list4x4,&pps->type_of_scaling_list_used[3]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[3],(mfxU8*) sps->ScalingLists4x4[3].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[3] = SCLDEFAULT;
                }
                // 4
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[4],(mfxU8*) default_inter_scaling_list4x4,&pps->type_of_scaling_list_used[4]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[4],(mfxU8*) pps->ScalingLists4x4[3].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[4] = SCLDEFAULT;
                }
                // 5
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[5],(mfxU8*) default_inter_scaling_list4x4,&pps->type_of_scaling_list_used[5]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[5],(mfxU8*) pps->ScalingLists4x4[4].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[5] = SCLDEFAULT;
                }

                if (pps->transform_8x8_mode_flag)
                {
                    // 0
                    if(Get1Bit())
                    {
                        GetScalingList8x8(&pps->ScalingLists8x8[0],(mfxU8*)default_intra_scaling_list8x8,&pps->type_of_scaling_list_used[6]);
                    }
                    else
                    {
                        FillScalingList8x8(&pps->ScalingLists8x8[0],(mfxU8*) sps->ScalingLists8x8[0].ScalingListCoeffs);
                        pps->type_of_scaling_list_used[6] = SCLDEFAULT;
                    }
                    // 1
                    if(Get1Bit())
                    {
                        GetScalingList8x8(&pps->ScalingLists8x8[1],(mfxU8*) default_inter_scaling_list8x8,&pps->type_of_scaling_list_used[7]);
                    }
                    else
                    {
                        FillScalingList8x8(&pps->ScalingLists8x8[1],(mfxU8*) sps->ScalingLists8x8[1].ScalingListCoeffs);
                        pps->type_of_scaling_list_used[7] = SCLDEFAULT;
                    }
                }
            }
            else
            {
                mfxI32 i;
                for(i=0; i<6; i++)
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[i],(mfxU8 *)sps->ScalingLists4x4[i].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[i] = sps->type_of_scaling_list_used[i];
                }

                if (pps->transform_8x8_mode_flag)
                {
                    for(i=0; i<2; i++)
                    {
                        FillScalingList8x8(&pps->ScalingLists8x8[i],(mfxU8 *)sps->ScalingLists8x8[i].ScalingListCoeffs);
                        pps->type_of_scaling_list_used[i] = sps->type_of_scaling_list_used[i];
                    }
                }
            }
        }
        else
        {
            //fall-back set rule A
            if(Get1Bit())
            {
                // 0
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[0],(mfxU8*)default_intra_scaling_list4x4,&pps->type_of_scaling_list_used[0]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[0],(mfxU8*) default_intra_scaling_list4x4);
                    pps->type_of_scaling_list_used[0] = SCLDEFAULT;
                }
                // 1
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[1],(mfxU8*) default_intra_scaling_list4x4,&pps->type_of_scaling_list_used[1]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[1],(mfxU8*) pps->ScalingLists4x4[0].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[1] = SCLDEFAULT;
                }
                // 2
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[2],(mfxU8*) default_intra_scaling_list4x4,&pps->type_of_scaling_list_used[2]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[2],(mfxU8*) pps->ScalingLists4x4[1].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[2] = SCLDEFAULT;
                }
                // 3
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[3],(mfxU8*)default_inter_scaling_list4x4,&pps->type_of_scaling_list_used[3]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[3],(mfxU8*) default_inter_scaling_list4x4);
                    pps->type_of_scaling_list_used[3] = SCLDEFAULT;
                }
                // 4
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[4],(mfxU8*) default_inter_scaling_list4x4,&pps->type_of_scaling_list_used[4]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[4],(mfxU8*) pps->ScalingLists4x4[3].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[4] = SCLDEFAULT;
                }
                // 5
                if(Get1Bit())
                {
                    GetScalingList4x4(&pps->ScalingLists4x4[5],(mfxU8*) default_inter_scaling_list4x4,&pps->type_of_scaling_list_used[5]);
                }
                else
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[5],(mfxU8*) pps->ScalingLists4x4[4].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[5] = SCLDEFAULT;
                }

                if (pps->transform_8x8_mode_flag)
                {
                    // 0
                    if(Get1Bit())
                    {
                        GetScalingList8x8(&pps->ScalingLists8x8[0],(mfxU8*)default_intra_scaling_list8x8,&pps->type_of_scaling_list_used[6]);
                    }
                    else
                    {
                        FillScalingList8x8(&pps->ScalingLists8x8[0],(mfxU8*) default_intra_scaling_list8x8);
                        pps->type_of_scaling_list_used[6] = SCLDEFAULT;
                    }
                    // 1
                    if(Get1Bit())
                    {
                        GetScalingList8x8(&pps->ScalingLists8x8[1],(mfxU8*) default_inter_scaling_list8x8,&pps->type_of_scaling_list_used[7]);
                    }
                    else
                    {
                        FillScalingList8x8(&pps->ScalingLists8x8[1],(mfxU8*) default_inter_scaling_list8x8);
                        pps->type_of_scaling_list_used[7] = SCLDEFAULT;
                    }
                }
            }
            else
            {
                mfxI32 i;
                for(i=0; i<6; i++)
                {
                    FillScalingList4x4(&pps->ScalingLists4x4[i],(mfxU8 *)sps->ScalingLists4x4[i].ScalingListCoeffs);
                    pps->type_of_scaling_list_used[i] = sps->type_of_scaling_list_used[i];
                }

                if (pps->transform_8x8_mode_flag)
                {
                    for(i=0; i<2; i++)
                    {
                        FillScalingList8x8(&pps->ScalingLists8x8[i],(mfxU8 *)sps->ScalingLists8x8[i].ScalingListCoeffs);
                        pps->type_of_scaling_list_used[i] = sps->type_of_scaling_list_used[i];
                    }
                }
            }
        }
        pps->chroma_qp_index_offset[1] = (mfxI8)GetVLCElement(true);
    }
    else
    {
        pps->chroma_qp_index_offset[1] = pps->chroma_qp_index_offset[0];
        mfxI32 i;
        for(i=0; i<6; i++)
        {
            FillScalingList4x4(&pps->ScalingLists4x4[i],(mfxU8 *)sps->ScalingLists4x4[i].ScalingListCoeffs);
            pps->type_of_scaling_list_used[i] = sps->type_of_scaling_list_used[i];
        }

        if (pps->transform_8x8_mode_flag)
        {
            for(i=0; i<2; i++)
            {
                FillScalingList8x8(&pps->ScalingLists8x8[i],(mfxU8 *)sps->ScalingLists8x8[i].ScalingListCoeffs);
                pps->type_of_scaling_list_used[i] = sps->type_of_scaling_list_used[i];
            }
        }
    }
    // calculate level scale matrices

    //start DC first
    //to do: reduce the number of matrices (in fact 1 is enough)
    mfxI32 i;
    // now process other 4x4 matrices
    for (i = 0; i < 6; i++)
    {
        for (mfxI32 j = 0; j < 88; j++)
            for (mfxI32 k = 0; k < 16; k++)
            {
                mfxU32 level_scale = pps->ScalingLists4x4[i].ScalingListCoeffs[k]*pre_norm_adjust4x4[j%6][pre_norm_adjust_index4x4[k]];
                pps->m_LevelScale4x4[i].LevelScaleCoeffs[j][k] = (mfxI16) level_scale;
            }
    }

    // process remaining 8x8  matrices
    for (i = 0; i < 2; i++)
    {
        for (mfxI32 j = 0; j < 88; j++)
            for (mfxI32 k = 0; k < 64; k++)
            {

                mfxU32 level_scale = pps->ScalingLists8x8[i].ScalingListCoeffs[k]*pre_norm_adjust8x8[j%6][pre_norm_adjust_index8x8[k]];
                    pps->m_LevelScale8x8[i].LevelScaleCoeffs[j][k] = (mfxI16) level_scale;

            }
    }

    return MFX_ERR_NONE;
}    // GetPictureParamSet

mfxStatus AVCHeadersBitstream::GetNalUnitPrefix(AVCNalExtension *pExt, mfxU32 )
{
    mfxStatus ps = MFX_ERR_NONE;

    ps = GetNalUnitExtension(pExt);

    if (ps != MFX_ERR_NONE || !pExt->svc_extension_flag)
        return ps;

    return ps;
}

mfxStatus AVCHeadersBitstream::GetNalUnitExtension(AVCNalExtension *pExt)
{
    pExt->extension_present = 1;

    // decode the type of the extension
    pExt->svc_extension_flag = (mfxU8) GetBits(1);

    // decode SVC extension
    if (pExt->svc_extension_flag)
    {
        pExt->svc.idr_flag = (mfxU8) Get1Bit();
        pExt->svc.priority_id = (mfxU8) GetBits(6);
        pExt->svc.no_inter_layer_pred_flag = (mfxU8) Get1Bit();
        pExt->svc.dependency_id = (mfxU8) GetBits(3);
        pExt->svc.quality_id = (mfxU8) GetBits(4);
        pExt->svc.temporal_id = (mfxU8) GetBits(3);
        pExt->svc.use_ref_base_pic_flag = (mfxU8) Get1Bit();
        pExt->svc.discardable_flag = (mfxU8) Get1Bit();
        pExt->svc.output_flag = (mfxU8) Get1Bit();
        GetBits(2);
    }
    // decode MVC extension
    else
    {
        pExt->mvc.non_idr_flag = (mfxU8) Get1Bit();
        pExt->mvc.priority_id = (mfxU16) GetBits(6);
        pExt->mvc.view_id = (mfxU16) GetBits(10);
        pExt->mvc.temporal_id = (mfxU8) GetBits(3);
        pExt->mvc.anchor_pic_flag = (mfxU8) Get1Bit();
        pExt->mvc.inter_view_flag = (mfxU8) Get1Bit();
        GetBits(1);
    }

    return MFX_ERR_NONE;

}

// ---------------------------------------------------------------------------
//    Read H.264 first part of slice header
//
//  Reading the rest of the header requires info in the picture and sequence
//  parameter sets referred to by this slice header.
//
//    Do not print debug messages when IsSearch is true. In that case the function
//    is being used to find the next compressed frame, errors may occur and should
//    not be reported.
//
// ---------------------------------------------------------------------------
mfxStatus AVCHeadersBitstream::GetSliceHeaderPart1(AVCSliceHeader *hdr)
{
    mfxU32 val;

    // decode NAL extension
    if (NAL_UT_CODED_SLICE_EXTENSION == hdr->nal_unit_type)
    {
        GetNalUnitExtension(&hdr->nal_ext);

        // set the IDR flag
        if (hdr->nal_ext.svc_extension_flag)
        {
            hdr->IdrPicFlag = hdr->nal_ext.svc.idr_flag;
        }
        else
        {
            hdr->view_id = hdr->nal_ext.mvc.view_id;
            hdr->IdrPicFlag = hdr->nal_ext.mvc.non_idr_flag ^ 1;
        }
    }
    else
    {
        hdr->IdrPicFlag = (NAL_UT_IDR_SLICE == hdr->nal_unit_type) ? (1) : (0);
        hdr->nal_ext.mvc.anchor_pic_flag = (mfxU8) hdr->IdrPicFlag ? 1 : 0;
        hdr->nal_ext.mvc.inter_view_flag = (mfxU8) 1;
    }

    hdr->first_mb_in_slice = GetVLCElement(false);
    if (0 > hdr->first_mb_in_slice) // upper bound is checked in AVCSlice
        return MFX_ERR_UNDEFINED_BEHAVIOR;

    // slice type
    val = GetVLCElement(false);
    if (val > S_INTRASLICE)
    {
        if (val > S_INTRASLICE + S_INTRASLICE + 1)
        {
            return MFX_ERR_UNDEFINED_BEHAVIOR;
        }
        else
        {
            // Slice type is specifying type of not only this but all remaining
            // slices in the picture. Since slice type is always present, this bit
            // of info is not used in our implementation. Adjust (just shift range)
            // and return type without this extra info.
            val -= (S_INTRASLICE + 1);
        }
    }

    if (val > INTRASLICE) // all other doesn't support
        return MFX_ERR_UNDEFINED_BEHAVIOR;

    hdr->slice_type = (EnumSliceCodType)val;

    mfxU32 pic_parameter_set_id = GetVLCElement(false);
    hdr->pic_parameter_set_id = (mfxU16)pic_parameter_set_id;
    if (pic_parameter_set_id > MAX_NUM_PIC_PARAM_SETS - 1)
    {
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    return MFX_ERR_NONE;
} // mfxStatus GetSliceHeaderPart1(AVCSliceHeader *pSliceHeader)

mfxStatus AVCHeadersBitstream::GetSliceHeaderPart2(
    AVCSliceHeader *hdr,        // slice header read goes here
    const AVCPicParamSet *pps,
    const AVCSeqParamSet *sps)            // from slice header NAL unit
{
    hdr->frame_num = GetBits(sps->log2_max_frame_num);

    hdr->bottom_field_flag = 0;
    if (sps->frame_mbs_only_flag == 0)
    {
        hdr->field_pic_flag = (mfxU8)Get1Bit();
        hdr->MbaffFrameFlag = !hdr->field_pic_flag && sps->mb_adaptive_frame_field_flag;
        if (hdr->field_pic_flag != 0)
        {
            hdr->bottom_field_flag = (mfxU8)Get1Bit();
        }
    }

    // correct frst_mb_in_slice in order to handle MBAFF
    if (hdr->MbaffFrameFlag && hdr->first_mb_in_slice)
        hdr->first_mb_in_slice <<= 1;

    if (hdr->IdrPicFlag)
    {
        mfxI32 pic_id = hdr->idr_pic_id = GetVLCElement(false);
        if (pic_id < 0 || pic_id > 65535)
            return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    if (sps->pic_order_cnt_type == 0)
    {
        hdr->pic_order_cnt_lsb = GetBits(sps->log2_max_pic_order_cnt_lsb);
        if (pps->pic_order_present_flag && (!hdr->field_pic_flag))
            hdr->delta_pic_order_cnt_bottom = GetVLCElement(true);
    }

    if ((sps->pic_order_cnt_type == 1) && (sps->delta_pic_order_always_zero_flag == 0))
    {
        hdr->delta_pic_order_cnt[0] = GetVLCElement(true);
        if (pps->pic_order_present_flag && (!hdr->field_pic_flag))
            hdr->delta_pic_order_cnt[1] = GetVLCElement(true);
    }

    if (pps->redundant_pic_cnt_present_flag)
    {
        // redundant pic count
        hdr->redundant_pic_cnt = GetVLCElement(false);
        if (hdr->redundant_pic_cnt > 127)
            return MFX_ERR_UNDEFINED_BEHAVIOR;
    }

    return MFX_ERR_NONE;
}

// ---------------------------------------------------------------------------
//    Read H.264 second part of slice header
//
//    Do not print debug messages when IsSearch is true. In that case the function
//    is being used to find the next compressed frame, errors may occur and should
//    not be reported.
// ---------------------------------------------------------------------------

mfxStatus AVCHeadersBitstream::GetSliceHeaderPart3(
    AVCSliceHeader *hdr,        // slice header read goes here
    PredWeightTable *pPredWeight_L0, // L0 weight table goes here
    PredWeightTable *pPredWeight_L1, // L1 weight table goes here
    RefPicListReorderInfo *pReorderInfo_L0,
    RefPicListReorderInfo *pReorderInfo_L1,
    AdaptiveMarkingInfo *pAdaptiveMarkingInfo,
    const AVCPicParamSet *pps,
    const AVCSeqParamSet *sps,
    mfxU8 NALRef_idc)            // from slice header NAL unit
{
    mfxU8 ref_pic_list_reordering_flag_l0 = 0;
    mfxU8 ref_pic_list_reordering_flag_l1 = 0;

    if (BPREDSLICE == hdr->slice_type)
    {
        // direct mode prediction method
        hdr->direct_spatial_mv_pred_flag = (mfxU8)Get1Bit();
    }

    if (PREDSLICE == hdr->slice_type ||
        S_PREDSLICE == hdr->slice_type ||
        BPREDSLICE == hdr->slice_type)
    {
        hdr->num_ref_idx_active_override_flag = (mfxU8)Get1Bit();
        if (hdr->num_ref_idx_active_override_flag != 0)
        // ref idx active l0 and l1
        {
            hdr->num_ref_idx_l0_active = GetVLCElement(false) + 1;
            if (BPREDSLICE == hdr->slice_type)
                hdr->num_ref_idx_l1_active = GetVLCElement(false) + 1;
        }
        else
        {
            // no overide, use num active from pic param set
            hdr->num_ref_idx_l0_active = pps->num_ref_idx_l0_active;
            if (BPREDSLICE == hdr->slice_type)
                hdr->num_ref_idx_l1_active = pps->num_ref_idx_l1_active;
            else
                hdr->num_ref_idx_l1_active = 0;
        }
    }    // ref idx override

    if (hdr->num_ref_idx_l1_active > MAX_NUM_REF_FRAMES || hdr->num_ref_idx_l0_active > MAX_NUM_REF_FRAMES)
        return MFX_ERR_UNDEFINED_BEHAVIOR;

    if (hdr->slice_type != INTRASLICE && hdr->slice_type != S_INTRASLICE)
    {
        mfxU32 reordering_of_pic_nums_idc;
        mfxU32 reorder_idx;

        // Reference picture list reordering
        ref_pic_list_reordering_flag_l0 = (mfxU8)Get1Bit();
        if (ref_pic_list_reordering_flag_l0)
        {
            bool bOk = true;

            reorder_idx = 0;
            reordering_of_pic_nums_idc = 0;

            // Get reorder idc,pic_num pairs until idc==3
            while (bOk)
            {
                reordering_of_pic_nums_idc = (mfxU8)GetVLCElement(false);
                if (reordering_of_pic_nums_idc > 5)
                    return MFX_ERR_UNDEFINED_BEHAVIOR;

                if (reordering_of_pic_nums_idc == 3)
                    break;

                if (reorder_idx >= MAX_NUM_REF_FRAMES)
                {
                    return MFX_ERR_UNDEFINED_BEHAVIOR;
                }

                pReorderInfo_L0->reordering_of_pic_nums_idc[reorder_idx] =
                                            (mfxU8)reordering_of_pic_nums_idc;
                pReorderInfo_L0->reorder_value[reorder_idx]  =
                                                    GetVLCElement(false);
                if (reordering_of_pic_nums_idc != 2)
                    // abs_diff_pic_num is coded minus 1
                    pReorderInfo_L0->reorder_value[reorder_idx]++;
                reorder_idx++;
            }    // while

            pReorderInfo_L0->num_entries = reorder_idx;
        }    // L0 reordering info
        else
            pReorderInfo_L0->num_entries = 0;

        if (BPREDSLICE == hdr->slice_type)
        {
            ref_pic_list_reordering_flag_l1 = (mfxU8)Get1Bit();
            if (ref_pic_list_reordering_flag_l1)
            {
                bool bOk = true;

                // Get reorder idc,pic_num pairs until idc==3
                reorder_idx = 0;
                reordering_of_pic_nums_idc = 0;
                while (bOk)
                {
                    reordering_of_pic_nums_idc = GetVLCElement(false);
                    if (reordering_of_pic_nums_idc > 5)
                        return MFX_ERR_UNDEFINED_BEHAVIOR;

                    if (reordering_of_pic_nums_idc == 3)
                        break;

                    if (reorder_idx >= MAX_NUM_REF_FRAMES)
                    {
                        return MFX_ERR_UNDEFINED_BEHAVIOR;
                    }

                    pReorderInfo_L1->reordering_of_pic_nums_idc[reorder_idx] =
                                                (mfxU8)reordering_of_pic_nums_idc;
                    pReorderInfo_L1->reorder_value[reorder_idx]  =
                                                        GetVLCElement(false);
                    if (reordering_of_pic_nums_idc != 2)
                        // abs_diff_pic_num is coded minus 1
                        pReorderInfo_L1->reorder_value[reorder_idx]++;
                    reorder_idx++;
                }    // while
                pReorderInfo_L1->num_entries = reorder_idx;
            }    // L1 reordering info
            else
                pReorderInfo_L1->num_entries = 0;

        }    // B slice
    }    // reordering info

    // prediction weight table
    if ( (pps->weighted_pred_flag &&
          ((PREDSLICE == hdr->slice_type) || (S_PREDSLICE == hdr->slice_type))) ||
         ((pps->weighted_bipred_idc == 1) && (BPREDSLICE == hdr->slice_type)))
    {
        hdr->luma_log2_weight_denom = (mfxU8)GetVLCElement(false);
        if (sps->chroma_format_idc != 0)
            hdr->chroma_log2_weight_denom = (mfxU8)GetVLCElement(false);

        for (mfxI32 refindex = 0; refindex < hdr->num_ref_idx_l0_active; refindex++)
        {
            pPredWeight_L0[refindex].luma_weight_flag = (mfxU8)Get1Bit();
            if (pPredWeight_L0[refindex].luma_weight_flag)
            {
                pPredWeight_L0[refindex].luma_weight = (mfxI8)GetVLCElement(true);
                pPredWeight_L0[refindex].luma_offset = (mfxI8)GetVLCElement(true);
            }
            else
            {
                pPredWeight_L0[refindex].luma_weight = (mfxI8)(1 << hdr->luma_log2_weight_denom);
                pPredWeight_L0[refindex].luma_offset = 0;
            }

            if (sps->chroma_format_idc != 0)
            {
                pPredWeight_L0[refindex].chroma_weight_flag = (mfxU8)Get1Bit();
                if (pPredWeight_L0[refindex].chroma_weight_flag)
                {
                    pPredWeight_L0[refindex].chroma_weight[0] = (mfxI8)GetVLCElement(true);
                    pPredWeight_L0[refindex].chroma_offset[0] = (mfxI8)GetVLCElement(true);
                    pPredWeight_L0[refindex].chroma_weight[1] = (mfxI8)GetVLCElement(true);
                    pPredWeight_L0[refindex].chroma_offset[1] = (mfxI8)GetVLCElement(true);
                }
                else
                {
                    pPredWeight_L0[refindex].chroma_weight[0] = (mfxI8)(1 << hdr->chroma_log2_weight_denom);
                    pPredWeight_L0[refindex].chroma_weight[1] = (mfxI8)(1 << hdr->chroma_log2_weight_denom);
                    pPredWeight_L0[refindex].chroma_offset[0] = 0;
                    pPredWeight_L0[refindex].chroma_offset[1] = 0;
                }
            }
        }

        if (BPREDSLICE == hdr->slice_type)
        {
            for (mfxI32 refindex = 0; refindex < hdr->num_ref_idx_l1_active; refindex++)
            {
                pPredWeight_L1[refindex].luma_weight_flag = (mfxU8)Get1Bit();
                if (pPredWeight_L1[refindex].luma_weight_flag)
                {
                    pPredWeight_L1[refindex].luma_weight = (mfxI8)GetVLCElement(true);
                    pPredWeight_L1[refindex].luma_offset = (mfxI8)GetVLCElement(true);
                }
                else
                {
                    pPredWeight_L1[refindex].luma_weight = (mfxI8)(1 << hdr->luma_log2_weight_denom);
                    pPredWeight_L1[refindex].luma_offset = 0;
                }

                if (sps->chroma_format_idc != 0)
                {
                    pPredWeight_L1[refindex].chroma_weight_flag = (mfxU8)Get1Bit();
                    if (pPredWeight_L1[refindex].chroma_weight_flag)
                    {
                        pPredWeight_L1[refindex].chroma_weight[0] = (mfxI8)GetVLCElement(true);
                        pPredWeight_L1[refindex].chroma_offset[0] = (mfxI8)GetVLCElement(true);
                        pPredWeight_L1[refindex].chroma_weight[1] = (mfxI8)GetVLCElement(true);
                        pPredWeight_L1[refindex].chroma_offset[1] = (mfxI8)GetVLCElement(true);
                    }
                    else
                    {
                        pPredWeight_L1[refindex].chroma_weight[0] = (mfxI8)(1 << hdr->chroma_log2_weight_denom);
                        pPredWeight_L1[refindex].chroma_weight[1] = (mfxI8)(1 << hdr->chroma_log2_weight_denom);
                        pPredWeight_L1[refindex].chroma_offset[0] = 0;
                        pPredWeight_L1[refindex].chroma_offset[1] = 0;
                    }
                }
            }
        }    // B slice
    }    // prediction weight table
    else
    {
        hdr->luma_log2_weight_denom = 0;
        hdr->chroma_log2_weight_denom = 0;
    }

    // dec_ref_pic_marking
    pAdaptiveMarkingInfo->num_entries = 0;

    if (NALRef_idc)
    {
        if (hdr->IdrPicFlag)
        {
            hdr->no_output_of_prior_pics_flag = (mfxU8)Get1Bit();
            hdr->long_term_reference_flag = (mfxU8)Get1Bit();
        }
        else
        {
            mfxU32 memory_management_control_operation;
            mfxU32 num_entries = 0;

            hdr->adaptive_ref_pic_marking_mode_flag = (mfxU8)Get1Bit();
            while (hdr->adaptive_ref_pic_marking_mode_flag != 0)
            {
                memory_management_control_operation = (mfxU8)GetVLCElement(false);
                if (memory_management_control_operation == 0)
                    break;

                if (memory_management_control_operation > 6)
                    return MFX_ERR_UNDEFINED_BEHAVIOR;

                pAdaptiveMarkingInfo->mmco[num_entries] =
                    (mfxU8)memory_management_control_operation;
                if (memory_management_control_operation != 5)
                     pAdaptiveMarkingInfo->value[num_entries*2] =
                        GetVLCElement(false);
                // Only mmco 3 requires 2 values
                if (memory_management_control_operation == 3)
                     pAdaptiveMarkingInfo->value[num_entries*2+1] =
                        GetVLCElement(false);
                num_entries++;
                if (num_entries >= MAX_NUM_REF_FRAMES)
                {
                    return MFX_ERR_UNDEFINED_BEHAVIOR;
                }
            }    // while
            pAdaptiveMarkingInfo->num_entries = num_entries;
        }
    }    // def_ref_pic_marking

    if (pps->entropy_coding_mode == 1  &&    // CABAC
        (hdr->slice_type != INTRASLICE && hdr->slice_type != S_INTRASLICE))
        hdr->cabac_init_idc = GetVLCElement(false);
    else
        hdr->cabac_init_idc = 0;

    if (hdr->cabac_init_idc > 2)
        return MFX_ERR_UNDEFINED_BEHAVIOR;

    hdr->slice_qp_delta = GetVLCElement(true);

    if (S_PREDSLICE == hdr->slice_type ||
        S_INTRASLICE == hdr->slice_type)
    {
        if (S_PREDSLICE == hdr->slice_type)
            hdr->sp_for_switch_flag = (mfxU8)Get1Bit();
        hdr->slice_qs_delta = GetVLCElement(true);
    }

    if (pps->deblocking_filter_variables_present_flag != 0)
    {
        // deblock filter flag and offsets
        hdr->disable_deblocking_filter_idc = GetVLCElement(false);
        if (hdr->disable_deblocking_filter_idc > 2)
            return MFX_ERR_UNDEFINED_BEHAVIOR;

        if (hdr->disable_deblocking_filter_idc != 1)
        {
            hdr->slice_alpha_c0_offset = GetVLCElement(true)<<1;
            hdr->slice_beta_offset = GetVLCElement(true)<<1;

            if (hdr->slice_alpha_c0_offset < -12 || hdr->slice_alpha_c0_offset > 12)
            {
                return MFX_ERR_UNDEFINED_BEHAVIOR;
            }

            if (hdr->slice_beta_offset < -12 || hdr->slice_beta_offset > 12)
            {
                return MFX_ERR_UNDEFINED_BEHAVIOR;
            }
        }
        else
        {
            // set filter offsets to max values to disable filter
            hdr->slice_alpha_c0_offset = (mfxI8)(0 - AVC_QP_MAX);
            hdr->slice_beta_offset = (mfxI8)(0 - AVC_QP_MAX);
        }
    }

    if (pps->num_slice_groups > 1 &&
        pps->SliceGroupInfo.slice_group_map_type >= 3 &&
        pps->SliceGroupInfo.slice_group_map_type <= 5)
    {
        mfxU32 num_bits;    // number of bits used to code slice_group_change_cycle
        mfxU32 val;
        mfxU32 pic_size_in_map_units;
        mfxU32 max_slice_group_change_cycle=0;

        // num_bits is Ceil(log2(picsizeinmapunits/slicegroupchangerate + 1))
        pic_size_in_map_units = sps->frame_width_in_mbs * sps->frame_height_in_mbs;

        max_slice_group_change_cycle = pic_size_in_map_units /
                        pps->SliceGroupInfo.t2.slice_group_change_rate;
        if (pic_size_in_map_units %
                        pps->SliceGroupInfo.t2.slice_group_change_rate)
            max_slice_group_change_cycle++;

        val = max_slice_group_change_cycle;
        num_bits = 0;
        while (val)
        {
            num_bits++;
            val >>= 1;
        }
        hdr->slice_group_change_cycle = GetBits(num_bits);
    }

    return MFX_ERR_NONE;
} // GetSliceHeaderPart3()

void AVCHeadersBitstream::GetScalingList4x4(AVCScalingList4x4 *scl, mfxU8 *def, mfxU8 *scl_type)
{
    mfxU32 lastScale = 8;
    mfxU32 nextScale = 8;
    bool DefaultMatrix = false;
    mfxI32 j;

    for (j = 0; j < 16; j++ )
    {
        if (nextScale != 0)
        {
            mfxI32 delta_scale  = GetVLCElement(true);
            if (delta_scale < -128 || delta_scale > 127)
                throw AVC_exception(MFX_ERR_UNDEFINED_BEHAVIOR);
            nextScale = ( lastScale + delta_scale + 256 ) & 0xff;
            DefaultMatrix = ( j == 0 && nextScale == 0 );
        }
        scl->ScalingListCoeffs[ mp_scan4x4[0][j] ] = ( nextScale == 0 ) ? (mfxU8)lastScale : (mfxU8)nextScale;
        lastScale = scl->ScalingListCoeffs[ mp_scan4x4[0][j] ];
    }
    if (!DefaultMatrix)
    {
        *scl_type = SCLREDEFINED;
        return;
    }
    *scl_type= SCLDEFAULT;
    FillScalingList4x4(scl,def);
    return;
}

void AVCHeadersBitstream::GetScalingList8x8(AVCScalingList8x8 *scl, mfxU8 *def, mfxU8 *scl_type)
{
    mfxU32 lastScale = 8;
    mfxU32 nextScale = 8;
    bool DefaultMatrix=false;
    mfxI32 j;

    for (j = 0; j < 64; j++ )
    {
        if (nextScale != 0)
        {
            mfxI32 delta_scale  = GetVLCElement(true);
            if (delta_scale < -128 || delta_scale > 127)
                throw AVC_exception(MFX_ERR_UNDEFINED_BEHAVIOR);
            nextScale = ( lastScale + delta_scale + 256 ) & 0xff;
            DefaultMatrix = ( j == 0 && nextScale == 0 );
        }
        scl->ScalingListCoeffs[ hp_scan8x8[0][j] ] = ( nextScale == 0 ) ? (mfxU8)lastScale : (mfxU8)nextScale;
        lastScale = scl->ScalingListCoeffs[ hp_scan8x8[0][j] ];
    }
    if (!DefaultMatrix)
    {
        *scl_type=SCLREDEFINED;
        return;
    }
    *scl_type= SCLDEFAULT;
    FillScalingList8x8(scl,def);
    return;

}

void SetDefaultScalingLists(AVCSeqParamSet * sps)
{
    mfxI32 i;

    for (i = 0; i < 6; i += 1)
    {
        FillFlatScalingList4x4(&sps->ScalingLists4x4[i]);
    }
    for (i = 0; i < 2; i += 1)
    {
        FillFlatScalingList8x8(&sps->ScalingLists8x8[i]);
    }
}

mfxStatus DecodeExpGolombOne(mfxU32 **ppBitStream, mfxI32 *pBitOffset,
                                                      mfxI32 *pDst,
                                                      mfxI32 isSigned)
{
    mfxU32 code;
    mfxU32 info     = 0;
    mfxI32 length   = 1;            /* for first bit read above*/
    mfxU32 thisChunksLength = 0;
    mfxU32 sval;

    /* Fast check for element = 0 */
    avcGetNBits((*ppBitStream), (*pBitOffset), 1, code);
    if (code)
    {
        *pDst = 0;
        return MFX_ERR_NONE;
    }

    avcGetNBits((*ppBitStream), (*pBitOffset), 8, code);
    length += 8;

    /* find nonzero byte */
    while (code == 0)
    {
        avcGetNBits((*ppBitStream), (*pBitOffset), 8, code);
        length += 8;
    }

    /* find leading '1' */
    while ((code & 0x80) == 0)
    {
        code <<= 1;
        thisChunksLength++;
    }
    length -= 8 - thisChunksLength;

    avcUngetNBits((*ppBitStream), (*pBitOffset), 8 - (thisChunksLength + 1));

    /* Get info portion of codeword */
    if (length)
    {
        avcGetNBits((*ppBitStream), (*pBitOffset),length, info);
    }

    sval = (1 << length) + info - 1;
    if (isSigned)
    {
        if (sval & 1)
            *pDst = (mfxI32) ((sval + 1) >> 1);
        else
            *pDst = -((mfxI32) (sval >> 1));
    }
    else
        *pDst = (mfxI32) sval;

    return MFX_ERR_NONE;
}

mfxI32 AVCHeadersBitstream::GetSEI(const HeaderSet<AVCSeqParamSet> & sps, mfxI32 current_sps, AVCSEIPayLoad *spl)
{
    mfxU32 code;
    mfxI32 payloadType = 0;

    avcNextBits(m_pbs, m_bitOffset, 8, code);
    while (code  ==  0xFF)
    {
        /* fixed-pattern bit string using 8 bits written equal to 0xFF */
        avcGetNBits(m_pbs, m_bitOffset, 8, code);
        payloadType += 255;
        avcNextBits(m_pbs, m_bitOffset, 8, code);
    }

    mfxI32 last_payload_type_byte;    //Ipp32u integer using 8 bits
    avcGetNBits(m_pbs, m_bitOffset, 8, last_payload_type_byte);

    payloadType += last_payload_type_byte;

    mfxI32 payloadSize = 0;

    avcNextBits(m_pbs, m_bitOffset, 8, code);
    while( code  ==  0xFF )
    {
        /* fixed-pattern bit string using 8 bits written equal to 0xFF */
        avcGetNBits(m_pbs, m_bitOffset, 8, code);
        payloadSize += 255;
        avcNextBits(m_pbs, m_bitOffset, 8, code);
    }

    mfxI32 last_payload_size_byte;    //Ipp32u integer using 8 bits

    avcGetNBits(m_pbs, m_bitOffset, 8, last_payload_size_byte);
    payloadSize += last_payload_size_byte;
    spl->Reset();
    spl->payLoadSize = payloadSize;

    if (payloadType < 0 || payloadType > SEI_RESERVED)
        payloadType = SEI_RESERVED;

    spl->payLoadType = (SEI_TYPE)payloadType;

    if (spl->payLoadSize > BytesLeft())
    {
        throw AVC_exception(MFX_ERR_UNDEFINED_BEHAVIOR);
    }

    mfxU32 * pbs;
    mfxU32 bitOffsetU;
    mfxI32 bitOffset;

    pbs       = m_pbs;
    bitOffsetU = m_bitOffset;
    bitOffset = bitOffsetU;

    mfxI32 ret = GetSEIPayload(sps, current_sps, spl);

    for (mfxU32 i = 0; i < spl->payLoadSize; i++)
    {
        avcSkipNBits(pbs, bitOffset, 8);
    }

    m_pbs = pbs;
    m_bitOffset = bitOffset;

    return ret;
}

mfxI32 AVCHeadersBitstream::GetSEIPayload(const HeaderSet<AVCSeqParamSet> & sps, mfxI32 current_sps, AVCSEIPayLoad *spl)
{
    switch (spl->payLoadType)
    {
    case SEI_RECOVERY_POINT_TYPE:
        return recovery_point(sps,current_sps,spl);
    default:
        return reserved_sei_message(sps,current_sps,spl);
    }
}

mfxI32 AVCHeadersBitstream::reserved_sei_message(const HeaderSet<AVCSeqParamSet> & , mfxI32 current_sps, AVCSEIPayLoad *spl)
{
    for (mfxU32 i = 0; i < spl->payLoadSize; i++)
        avcSkipNBits(m_pbs, m_bitOffset, 8)
    AlignPointerRight();
    return current_sps;
}

mfxI32 AVCHeadersBitstream::recovery_point(const HeaderSet<AVCSeqParamSet> & , mfxI32 current_sps, AVCSEIPayLoad *spl)
{
    AVCSEIPayLoad::SEIMessages::RecoveryPoint * recPoint = &(spl->SEI_messages.recovery_point);

    recPoint->recovery_frame_cnt = (mfxU8)GetVLCElement(false);

    recPoint->exact_match_flag = (mfxU8)Get1Bit();
    recPoint->broken_link_flag = (mfxU8)Get1Bit();
    recPoint->changing_slice_group_idc = (mfxU8)GetBits(2);

    if (recPoint->changing_slice_group_idc > 2)
        return -1;

    return current_sps;
}

void HeapObject::Free()
{
}

} // namespace ProtectedLibrary
