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

#ifndef __AVC_STRUCTURES_H__
#define __AVC_STRUCTURES_H__

#if defined(_WIN32) || defined(_WIN64)
#include <windows.h>
#endif
#include <string>
#include <string.h>
#include <vector>
#include "mfxstructures.h"

namespace ProtectedLibrary
{

enum
{
    AVC_PROFILE_BASELINE           = 66,
    AVC_PROFILE_MAIN               = 77,
    AVC_PROFILE_SCALABLE_BASELINE  = 83,
    AVC_PROFILE_SCALABLE_HIGH      = 86,
    AVC_PROFILE_EXTENDED           = 88,
    AVC_PROFILE_HIGH               = 100,
    AVC_PROFILE_HIGH10             = 110,
    AVC_PROFILE_MULTIVIEW_HIGH     = 118,
    AVC_PROFILE_HIGH422            = 122,
    AVC_PROFILE_STEREO_HIGH        = 128,
    AVC_PROFILE_HIGH444            = 144,
    AVC_PROFILE_ADVANCED444_INTRA  = 166,
    AVC_PROFILE_ADVANCED444        = 188
};

enum
{
    AVC_LEVEL_1    = 10,
    AVC_LEVEL_11   = 11,
    AVC_LEVEL_1b   = 11,
    AVC_LEVEL_12   = 12,
    AVC_LEVEL_13   = 13,

    AVC_LEVEL_2    = 20,
    AVC_LEVEL_21   = 21,
    AVC_LEVEL_22   = 22,

    AVC_LEVEL_3    = 30,
    AVC_LEVEL_31   = 31,
    AVC_LEVEL_32   = 32,

    AVC_LEVEL_4    = 40,
    AVC_LEVEL_41   = 41,
    AVC_LEVEL_42   = 42,

    AVC_LEVEL_5    = 50,
    AVC_LEVEL_51   = 51,
    AVC_LEVEL_MAX  = 51,

    AVC_LEVEL_9    = 9  // for SVC profiles
};

// Although the standard allows for a minimum width or height of 4, this
// implementation restricts the minimum value to 32.

enum    // Valid QP range
{
    AVC_QP_MAX = 51,
    AVC_QP_MIN = 0
};

enum {
    FLD_STRUCTURE       = 0,
    TOP_FLD_STRUCTURE   = 0,
    BOTTOM_FLD_STRUCTURE = 1,
    FRM_STRUCTURE   = 2,
    AFRM_STRUCTURE  = 3
};

enum DisplayPictureStruct {
    DPS_FRAME     = 0,
    DPS_TOP,         // one field
    DPS_BOTTOM,      // one field
    DPS_TOP_BOTTOM,
    DPS_BOTTOM_TOP,
    DPS_TOP_BOTTOM_TOP,
    DPS_BOTTOM_TOP_BOTTOM,
    DPS_FRAME_DOUBLING,
    DPS_FRAME_TRIPLING
};

typedef enum {
    NAL_UT_UNSPECIFIED  = 0, // Unspecified
    NAL_UT_SLICE     = 1, // Coded Slice - slice_layer_no_partioning_rbsp
    NAL_UT_DPA       = 2, // Coded Data partition A - dpa_layer_rbsp
    NAL_UT_DPB       = 3, // Coded Data partition A - dpa_layer_rbsp
    NAL_UT_DPC       = 4, // Coded Data partition A - dpa_layer_rbsp
    NAL_UT_IDR_SLICE = 5, // Coded Slice of a IDR Picture - slice_layer_no_partioning_rbsp
    NAL_UT_SEI       = 6, // Supplemental Enhancement Information - sei_rbsp
    NAL_UT_SPS       = 7, // Sequence Parameter Set - seq_parameter_set_rbsp
    NAL_UT_PPS       = 8, // Picture Parameter Set - pic_parameter_set_rbsp
    NAL_UT_AUD        = 9, // Access Unit Delimiter - access_unit_delimiter_rbsp
    NAL_END_OF_SEQ   = 10, // End of sequence end_of_seq_rbsp()
    NAL_END_OF_STREAM = 11, // End of stream end_of_stream_rbsp
    NAL_UT_FD        = 12, // Filler Data - filler_data_rbsp
    NAL_UT_SPS_EX    = 13, // Sequence Parameter Set Extension - seq_parameter_set_extension_rbsp
    NAL_UNIT_PREFIX  = 14, // Prefix NAL unit in scalable extension - prefix_nal_unit_rbsp
    NAL_UNIT_SUBSET_SPS = 15, // Subset Sequence Parameter Set - subset_seq_parameter_set_rbsp
    NAL_UT_AUXILIARY = 19, // Auxiliary coded picture
    NAL_UT_CODED_SLICE_EXTENSION = 20 // Coded slice in scalable extension - slice_layer_in_scalable_extension_rbsp
} NAL_Unit_Type;

// Note!  The Picture Code Type values below are no longer used in the
// core encoder.   It only knows about slice types, and whether or not
// the frame is IDR, Reference or Disposable.  See enum above.

enum EnumSliceCodType        // Permitted MB Prediction Types
{                        // ------------------------------------
    PREDSLICE      = 0,    // I (Intra), P (Pred)
    BPREDSLICE     = 1, // I, P, B (BiPred)
    INTRASLICE     = 2,    // I
    S_PREDSLICE    = 3,    // SP (SPred), I
    S_INTRASLICE   = 4    // SI (SIntra), I
};

typedef enum
{
    SEI_BUFFERING_PERIOD_TYPE   = 0,
    SEI_PIC_TIMING_TYPE         = 1,
    SEI_PAN_SCAN_RECT_TYPE      = 2,
    SEI_FILLER_TYPE             = 3,
    SEI_USER_DATA_REGISTERED_TYPE   = 4,
    SEI_USER_DATA_UNREGISTERED_TYPE = 5,
    SEI_RECOVERY_POINT_TYPE         = 6,
    SEI_DEC_REF_PIC_MARKING_TYPE    = 7,
    SEI_SPARE_PIC_TYPE              = 8,
    SEI_SCENE_INFO_TYPE             = 9,
    SEI_SUB_SEQ_INFO_TYPE           = 10,
    SEI_SUB_SEQ_LAYER_TYPE          = 11,
    SEI_SUB_SEQ_TYPE                = 12,
    SEI_FULL_FRAME_FREEZE_TYPE      = 13,
    SEI_FULL_FRAME_FREEZE_RELEASE_TYPE  = 14,
    SEI_FULL_FRAME_SNAPSHOT_TYPE        = 15,
    SEI_PROGRESSIVE_REF_SEGMENT_START_TYPE  = 16,
    SEI_PROGRESSIVE_REF_SEGMENT_END_TYPE    = 17,
    SEI_MOTION_CONSTRAINED_SG_SET_TYPE      = 18,
    SEI_RESERVED                            = 19
} SEI_TYPE;


#define IS_I_SLICE(SliceType) ((SliceType) == INTRASLICE)
#define IS_P_SLICE(SliceType) ((SliceType) == PREDSLICE || (SliceType) == S_PREDSLICE)
#define IS_B_SLICE(SliceType) ((SliceType) == BPREDSLICE)

enum
{
    MAX_NUM_SEQ_PARAM_SETS = 32,
    MAX_NUM_PIC_PARAM_SETS = 256,

    MAX_SLICE_NUM       = 128, //INCREASE IF NEEDED OR SET to -1 for adaptive counting (increases memory usage)
    MAX_NUM_REF_FRAMES  = 32,

    MAX_REF_FRAMES_IN_POC_CYCLE = 256,

    MAX_NUM_SLICE_GROUPS        = 8,
    MAX_SLICE_GROUP_MAP_TYPE    = 6,

    NUM_INTRA_TYPE_ELEMENTS     = 16,

    COEFFICIENTS_BUFFER_SIZE    = 16 * 51,

    MINIMAL_DATA_SIZE           = 4
};

// Possible values for disable_deblocking_filter_idc:
enum DeblockingModes_t
{
    DEBLOCK_FILTER_ON                   = 0,
    DEBLOCK_FILTER_OFF                  = 1,
    DEBLOCK_FILTER_ON_NO_SLICE_EDGES    = 2
};

#pragma pack(1)

struct AVCScalingList4x4
{
    mfxU8 ScalingListCoeffs[16];
};

struct AVCScalingList8x8
{
    mfxU8 ScalingListCoeffs[64];
};

struct AVCWholeQPLevelScale4x4
{
    mfxI16 LevelScaleCoeffs[88]/*since we do not support 422 and 444*/[16];
};
struct AVCWholeQPLevelScale8x8
{
    mfxI16 LevelScaleCoeffs[88]/*since we do not support 422 and 444*/[64];
};


//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Memory class
//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
class RefCounter
{
public:

    RefCounter() : m_refCounter(0)
    {
    }

    virtual ~RefCounter()
    {
    }

    void IncrementReference() const
    {
        m_refCounter++;
    }

    void DecrementReference()
    {
        m_refCounter--;

        if (!m_refCounter)
        {
            Free();
        }
    }

    void ResetRefCounter() {m_refCounter = 0;}

    mfxU32 GetRefCounter() {return m_refCounter;}

protected:
    mutable mfxI32 m_refCounter;

    virtual void Free()
    {
    }
};

class HeapObject : public RefCounter
{
public:

    virtual ~HeapObject() {}

    virtual void Reset()
    {
    }

    virtual void Free();
};

#pragma pack()

#pragma pack(16)

typedef mfxU32 IntraType;

// Sequence parameter set structure, corresponding to the H.264 bitstream definition.
struct AVCSeqParamSetBase
{
    mfxU8        profile_idc;                        // baseline, main, etc.
    mfxU8        level_idc;
    mfxU8        constrained_set0_flag;
    mfxU8        constrained_set1_flag;
    mfxU8        constrained_set2_flag;
    mfxU8        constrained_set3_flag;
    mfxU8        chroma_format_idc;
    mfxU8        residual_colour_transform_flag;
    mfxU8        bit_depth_luma;
    mfxU8        bit_depth_chroma;
    mfxU8        qpprime_y_zero_transform_bypass_flag;
    mfxU8        type_of_scaling_list_used[8];
    mfxU8        seq_scaling_matrix_present_flag;
    AVCScalingList4x4 ScalingLists4x4[6];
    AVCScalingList8x8 ScalingLists8x8[2];
    mfxU8        gaps_in_frame_num_value_allowed_flag;
    mfxU8        frame_cropping_flag;
    mfxU32       frame_cropping_rect_left_offset;
    mfxU32       frame_cropping_rect_right_offset;
    mfxU32       frame_cropping_rect_top_offset;
    mfxU32       frame_cropping_rect_bottom_offset;
    mfxU8        more_than_one_slice_group_allowed_flag;
    mfxU8        arbitrary_slice_order_allowed_flag;  // If zero, slice order in pictures must
                                                      // be in increasing MB address order.
    mfxU8        redundant_pictures_allowed_flag;
    mfxU8        seq_parameter_set_id;                // id of this sequence parameter set
    mfxU8        log2_max_frame_num;                  // Number of bits to hold the frame_num
    mfxU8        pic_order_cnt_type;                  // Picture order counting method

    mfxU8        delta_pic_order_always_zero_flag;    // If zero, delta_pic_order_cnt fields are
                                                      // present in slice header.
    mfxU8        frame_mbs_only_flag;                 // Nonzero indicates all pictures in sequence
                                                      // are coded as frames (not fields).
    mfxU8        required_frame_num_update_behavior_flag;

    mfxU8        mb_adaptive_frame_field_flag;        // Nonzero indicates frame/field switch
                                                      // at macroblock level
    mfxU8        direct_8x8_inference_flag;           // Direct motion vector derivation method
    mfxU8        vui_parameters_present_flag;         // Zero indicates default VUI parameters
    mfxU32       log2_max_pic_order_cnt_lsb;          // Value of MaxPicOrderCntLsb.
    mfxI32       offset_for_non_ref_pic;

    mfxI32       offset_for_top_to_bottom_field;      // Expected pic order count difference from
                                                      // top field to bottom field.

    mfxU32       num_ref_frames_in_pic_order_cnt_cycle;
    mfxU32       num_ref_frames;                      // total number of pics in decoded pic buffer
    mfxU32       frame_width_in_mbs;
    mfxU32       frame_height_in_mbs;

    // These fields are calculated from values above.  They are not written to the bitstream
    mfxU32       MaxMbAddress;
    mfxU32       MaxPicOrderCntLsb;
    // vui part
    mfxU8        aspect_ratio_info_present_flag;
    mfxU8        aspect_ratio_idc;
    mfxU16       sar_width;
    mfxU16       sar_height;
    mfxU8        overscan_info_present_flag;
    mfxU8        overscan_appropriate_flag;
    mfxU8        video_signal_type_present_flag;
    mfxU8        video_format;
    mfxU8        video_full_range_flag;
    mfxU8        colour_description_present_flag;
    mfxU8        colour_primaries;
    mfxU8        transfer_characteristics;
    mfxU8        matrix_coefficients;
    mfxU8        chroma_loc_info_present_flag;
    mfxU8        chroma_sample_loc_type_top_field;
    mfxU8        chroma_sample_loc_type_bottom_field;
    mfxU8        timing_info_present_flag;
    mfxU32       num_units_in_tick;
    mfxU32       time_scale;
    mfxU8        fixed_frame_rate_flag;
    mfxU8        nal_hrd_parameters_present_flag;
    mfxU8        vcl_hrd_parameters_present_flag;
    mfxU8        low_delay_hrd_flag;
    mfxU8        pic_struct_present_flag;
    mfxU8        bitstream_restriction_flag;
    mfxU8        motion_vectors_over_pic_boundaries_flag;
    mfxU8        max_bytes_per_pic_denom;
    mfxU8        max_bits_per_mb_denom;
    mfxU8        log2_max_mv_length_horizontal;
    mfxU8        log2_max_mv_length_vertical;
    mfxU8        num_reorder_frames;
    mfxU8        max_dec_frame_buffering;
    //hrd_parameters
    mfxU8        cpb_cnt;
    mfxU8        bit_rate_scale;
    mfxU8        cpb_size_scale;
    mfxU32       bit_rate_value[32];
    mfxU32       cpb_size_value[32];
    mfxU8        cbr_flag[32];
    mfxU8        initial_cpb_removal_delay_length;
    mfxU8        cpb_removal_delay_length;
    mfxU8        dpb_output_delay_length;
    mfxU8        time_offset_length;

    mfxI32       poffset_for_ref_frame[MAX_REF_FRAMES_IN_POC_CYCLE];  // for pic order cnt type 1
                                                      // length num_stored_frames_in_pic_order_cnt_cycle,
    void Reset()
    {
        AVCSeqParamSetBase sps = {0};
        *this = sps;
    }

};    // AVCSeqParamSetBase

// Sequence parameter set structure, corresponding to the H.264 bitstream definition.
struct AVCSeqParamSet : public HeapObject, public AVCSeqParamSetBase
{
    AVCSeqParamSet()
        : HeapObject()
        , AVCSeqParamSetBase()
    {
        Reset();
    }

    ~AVCSeqParamSet()
    {
    }

    mfxI32 GetID() const
    {
        return seq_parameter_set_id;
    }

    virtual void Reset()
    {
        AVCSeqParamSetBase::Reset();

        seq_parameter_set_id = MAX_NUM_SEQ_PARAM_SETS;

        // set some parameters by default
        video_format = 5; // unspecified
        video_full_range_flag = 0;
        colour_primaries = 2; // unspecified
        transfer_characteristics = 2; // unspecified
        matrix_coefficients = 2; // unspecified
    }
};    // AVCSeqParamSet

// Sequence parameter set extension structure, corresponding to the H.264 bitstream definition.
struct AVCSeqParamSetExtension
{
    mfxU8       seq_parameter_set_id;
    mfxU8       aux_format_idc;
    mfxU8       bit_depth_aux;
    mfxU8       alpha_incr_flag;
    mfxU8       alpha_opaque_value;
    mfxU8       alpha_transparent_value;
    mfxU8       additional_extension_flag;

    AVCSeqParamSetExtension()
    {
        Reset();
    }

    virtual void Reset()
    {
        aux_format_idc = 0;
        seq_parameter_set_id = MAX_NUM_SEQ_PARAM_SETS;    // illegal id
        bit_depth_aux = 0;
        alpha_incr_flag = 0;
        alpha_opaque_value = 0;
        alpha_transparent_value = 0;
        additional_extension_flag = 0;
    }

    mfxI32 GetID() const
    {
        return seq_parameter_set_id;
    }

    virtual ~AVCSeqParamSetExtension(){}

};    // AVCSeqParamSetExtension

// Picture parameter set structure, corresponding to the H.264 bitstream definition.
struct AVCPicParamSetBase
{
// Flexible macroblock order structure, defining the FMO map for a picture
// paramter set.

    struct SliceGroupInfoStruct
    {
        mfxU8        slice_group_map_type;                // 0..6

        // The additional slice group data depends upon map type
        union
        {
            // type 0
            mfxU32    run_length[MAX_NUM_SLICE_GROUPS];

            // type 2
            struct
            {
                mfxU32 top_left[MAX_NUM_SLICE_GROUPS-1];
                mfxU32 bottom_right[MAX_NUM_SLICE_GROUPS-1];
            }t1;

            // types 3-5
            struct
            {
                mfxU8  slice_group_change_direction_flag;
                mfxU32 slice_group_change_rate;
            }t2;

            // type 6
            struct
            {
                mfxU32 pic_size_in_map_units;     // number of macroblocks if no field coding
            }t3;
        };

        std::vector<mfxU8> pSliceGroupIDMap;          // Id for each slice group map unit (for t3 struct of)
    };    // SliceGroupInfoStruct

    mfxU16       pic_parameter_set_id;            // of this picture parameter set
    mfxU8        seq_parameter_set_id;            // of seq param set used for this pic param set
    mfxU8        entropy_coding_mode;             // zero: CAVLC, else CABAC

    mfxU8        pic_order_present_flag;          // Zero indicates only delta_pic_order_cnt[0] is
                                                  // present in slice header; nonzero indicates
                                                  // delta_pic_order_cnt[1] is also present.

    mfxU8        weighted_pred_flag;              // Nonzero indicates weighted prediction applied to
                                                  // P and SP slices
    mfxU8        weighted_bipred_idc;             // 0: no weighted prediction in B slices
                                                  // 1: explicit weighted prediction
                                                  // 2: implicit weighted prediction
    mfxI8        pic_init_qp;                     // default QP for I,P,B slices
    mfxI8        pic_init_qs;                     // default QP for SP, SI slices

    mfxI8        chroma_qp_index_offset[2];       // offset to add to QP for chroma

    mfxU8        deblocking_filter_variables_present_flag;    // If nonzero, deblock filter params are
                                                  // present in the slice header.
    mfxU8        constrained_intra_pred_flag;     // Nonzero indicates constrained intra mode

    mfxU8        redundant_pic_cnt_present_flag;  // Nonzero indicates presence of redundant_pic_cnt
                                                  // in slice header
    mfxU32       num_slice_groups;                // One: no FMO
    mfxU32       num_ref_idx_l0_active;           // num of ref pics in list 0 used to decode the picture
    mfxU32       num_ref_idx_l1_active;           // num of ref pics in list 1 used to decode the picture
    mfxU8        transform_8x8_mode_flag;
    mfxU8        type_of_scaling_list_used[8];

    AVCScalingList4x4 ScalingLists4x4[6];
    AVCScalingList8x8 ScalingLists8x8[2];

    // Level Scale addition
    AVCWholeQPLevelScale4x4        m_LevelScale4x4[6];
    AVCWholeQPLevelScale8x8        m_LevelScale8x8[2];

    SliceGroupInfoStruct SliceGroupInfo;    // Used only when num_slice_groups > 1

    void Reset()
    {
        AVCPicParamSetBase pps = {0};
        *this = pps;
    }
};    // AVCPicParamSet

// Picture parameter set structure, corresponding to the H.264 bitstream definition.
struct AVCPicParamSet : public HeapObject, public AVCPicParamSetBase
{
    AVCPicParamSet()
        : AVCPicParamSetBase()
    {
        Reset();
    }

    void Reset()
    {
        AVCPicParamSetBase::Reset();

        pic_parameter_set_id = MAX_NUM_PIC_PARAM_SETS;
        seq_parameter_set_id = MAX_NUM_SEQ_PARAM_SETS;
        num_slice_groups = 0;
        SliceGroupInfo.pSliceGroupIDMap.clear();
    }

    ~AVCPicParamSet()
    {
    }

    mfxI32 GetID() const
    {
        return pic_parameter_set_id;
    }

};    // H264PicParamSet

struct RefPicListReorderInfo
{
    mfxU32       num_entries;                 // number of currently valid idc,value pairs
    mfxU8        reordering_of_pic_nums_idc[MAX_NUM_REF_FRAMES];
    mfxU32       reorder_value[MAX_NUM_REF_FRAMES];    // abs_diff_pic_num or long_term_pic_num
};

struct AdaptiveMarkingInfo
{
    mfxU32       num_entries;                 // number of currently valid mmco,value pairs
    mfxU8        mmco[MAX_NUM_REF_FRAMES];    // memory management control operation id
    mfxU32       value[MAX_NUM_REF_FRAMES*2]; // operation-dependent data, max 2 per operation
};

struct PredWeightTable
{
    mfxU8        luma_weight_flag;            // nonzero: luma weight and offset in bitstream
    mfxU8        chroma_weight_flag;          // nonzero: chroma weight and offset in bitstream
    mfxI8        luma_weight;                 // luma weighting factor
    mfxI8        luma_offset;                 // luma weighting offset
    mfxI8        chroma_weight[2];            // chroma weighting factor (Cb,Cr)
    mfxI8        chroma_offset[2];            // chroma weighting offset (Cb,Cr)
};    // PredWeightTable

typedef mfxI32 AVCDecoderMBAddr;
// NAL unit SVC extension structure
struct AVCNalSvcExtension
{
    mfxU8 idr_flag;
    mfxU8 priority_id;
    mfxU8 no_inter_layer_pred_flag;
    mfxU8 dependency_id;
    mfxU8 quality_id;
    mfxU8 temporal_id;
    mfxU8 use_ref_base_pic_flag;
    mfxU8 discardable_flag;
    mfxU8 output_flag;

    mfxU8 store_ref_base_pic_flag;
    mfxU8 adaptive_ref_base_pic_marking_mode_flag;
    AdaptiveMarkingInfo adaptiveMarkingInfo;
};

// NAL unit SVC extension structure
struct AVCNalMvcExtension
{
    mfxU8 non_idr_flag;
    mfxU16 priority_id;
    // view_id variable is duplicated to the slice header itself
    mfxU16 view_id;
    mfxU8 temporal_id;
    mfxU8 anchor_pic_flag;
    mfxU8 inter_view_flag;

    mfxU8 padding[3];
};

// NAL unit extension structure
struct AVCNalExtension
{
    mfxU8 extension_present;

    // equal to 1 specifies that NAL extension contains SVC related parameters
    mfxU8 svc_extension_flag;

    union
    {
        AVCNalSvcExtension svc;
        AVCNalMvcExtension mvc;
    };
};

// Slice header structure, corresponding to the H.264 bitstream definition.
struct AVCSliceHeader
{
    // flag equal 1 means that the slice belong to IDR or anchor access unit
    mfxU32 IdrPicFlag;

    // specified that NAL unit contains any information accessed from
    // the decoding process of other NAL units.
    mfxU8  nal_ref_idc;
    // specifies the type of RBSP data structure contained in the NAL unit as
    // specified in Table 7-1 of h264 standard
    NAL_Unit_Type nal_unit_type;

    // NAL unit extension parameters
    AVCNalExtension nal_ext;
    mfxU32 view_id;

    mfxU16        pic_parameter_set_id;                 // of pic param set used for this slice
    mfxU8         field_pic_flag;                       // zero: frame picture, else field picture
    mfxU8         MbaffFrameFlag;
    mfxU8         bottom_field_flag;                    // zero: top field, else bottom field
    mfxU8         direct_spatial_mv_pred_flag;          // zero: temporal direct, else spatial direct
    mfxU8         num_ref_idx_active_override_flag;     // nonzero: use ref_idx_active from slice header
                                                        // instead of those from pic param set
    mfxU8         no_output_of_prior_pics_flag;         // nonzero: remove previously decoded pictures
                                                        // from decoded picture buffer
    mfxU8         long_term_reference_flag;             // How to set MaxLongTermFrameIdx
    mfxU32        cabac_init_idc;                      // CABAC initialization table index (0..2)
    mfxU8         adaptive_ref_pic_marking_mode_flag;   // Ref pic marking mode of current picture
    mfxI32        slice_qp_delta;                       // to calculate default slice QP
    mfxU8         sp_for_switch_flag;                   // SP slice decoding control
    mfxI32        slice_qs_delta;                       // to calculate default SP,SI slice QS
    mfxU32        disable_deblocking_filter_idc;       // deblock filter control, 0=filter all edges
    mfxI32        slice_alpha_c0_offset;               // deblock filter c0, alpha table offset
    mfxI32        slice_beta_offset;                   // deblock filter beta table offset
    AVCDecoderMBAddr first_mb_in_slice;
    mfxI32        frame_num;
    EnumSliceCodType slice_type;
    mfxU32        idr_pic_id;                           // ID of an IDR picture
    mfxI32        pic_order_cnt_lsb;                    // picture order count (mod MaxPicOrderCntLsb)
    mfxI32        delta_pic_order_cnt_bottom;           // Pic order count difference, top & bottom fields
    mfxU32        difference_of_pic_nums;               // Ref pic memory mgmt
    mfxU32        long_term_pic_num;                    // Ref pic memory mgmt
    mfxU32        long_term_frame_idx;                  // Ref pic memory mgmt
    mfxU32        max_long_term_frame_idx;              // Ref pic memory mgmt
    mfxI32        delta_pic_order_cnt[2];               // picture order count differences
    mfxU32        redundant_pic_cnt;                    // for redundant slices
    mfxI32        num_ref_idx_l0_active;                // num of ref pics in list 0 used to decode the slice,
                                                        // see num_ref_idx_active_override_flag
    mfxI32        num_ref_idx_l1_active;                // num of ref pics in list 1 used to decode the slice
                                                        // see num_ref_idx_active_override_flag
    mfxU32        slice_group_change_cycle;             // for FMO
    mfxU8         luma_log2_weight_denom;               // luma weighting denominator
    mfxU8         chroma_log2_weight_denom;             // chroma weighting denominator

    bool          is_auxiliary;
}; // AVCSliceHeader


struct AVCSEIPayLoadBase
{
    SEI_TYPE payLoadType;
    mfxU32   payLoadSize;

    union SEIMessages
    {
        struct BufferingPeriod
        {
            mfxU32 initial_cbp_removal_delay[2][16];
            mfxU32 initial_cbp_removal_delay_offset[2][16];
        }buffering_period;

        struct PicTiming
        {
            mfxU32 cbp_removal_delay;
            mfxU32 dpb_ouput_delay;
            DisplayPictureStruct pic_struct;
            mfxU8  clock_timestamp_flag[16];
            struct ClockTimestamps
            {
                mfxU8 ct_type;
                mfxU8 nunit_field_based_flag;
                mfxU8 counting_type;
                mfxU8 full_timestamp_flag;
                mfxU8 discontinuity_flag;
                mfxU8 cnt_dropped_flag;
                mfxU8 n_frames;
                mfxU8 seconds_value;
                mfxU8 minutes_value;
                mfxU8 hours_value;
                mfxU8 time_offset;
            }clock_timestamps[16];
        }pic_timing;

        struct PanScanRect
        {
            mfxU8  pan_scan_rect_id;
            mfxU8  pan_scan_rect_cancel_flag;
            mfxU8  pan_scan_cnt;
            mfxU32 pan_scan_rect_left_offset[32];
            mfxU32 pan_scan_rect_right_offset[32];
            mfxU32 pan_scan_rect_top_offset[32];
            mfxU32 pan_scan_rect_bottom_offset[32];
            mfxU8  pan_scan_rect_repetition_period;
        }pan_scan_rect;

        struct UserDataRegistered
        {
            mfxU8 itu_t_t35_country_code;
            mfxU8 itu_t_t35_country_code_extension_byte;
        } user_data_registered;

        struct RecoveryPoint
        {
            mfxU8 recovery_frame_cnt;
            mfxU8 exact_match_flag;
            mfxU8 broken_link_flag;
            mfxU8 changing_slice_group_idc;
        }recovery_point;

        struct DecRefPicMarkingRepetition
        {
            mfxU8 original_idr_flag;
            mfxU8 original_frame_num;
            mfxU8 original_field_pic_flag;
            mfxU8 original_bottom_field_flag;
            mfxU8 long_term_reference_flag;
            AdaptiveMarkingInfo adaptiveMarkingInfo;
        }dec_ref_pic_marking_repetition;

        struct SparePic
        {
            mfxU32 target_frame_num;
            mfxU8  spare_field_flag;
            mfxU8  target_bottom_field_flag;
            mfxU8  num_spare_pics;
            mfxU8  delta_spare_frame_num[16];
            mfxU8  spare_bottom_field_flag[16];
            mfxU8  spare_area_idc[16];
            mfxU8  *spare_unit_flag[16];
            mfxU8  *zero_run_length[16];
        }spare_pic;

        struct SceneInfo
        {
            mfxU8 scene_info_present_flag;
            mfxU8 scene_id;
            mfxU8 scene_transition_type;
            mfxU8 second_scene_id;
        }scene_info;

        struct SubSeqInfo
        {
            mfxU8 sub_seq_layer_num;
            mfxU8 sub_seq_id;
            mfxU8 first_ref_pic_flag;
            mfxU8 leading_non_ref_pic_flag;
            mfxU8 last_pic_flag;
            mfxU8 sub_seq_frame_num_flag;
            mfxU8 sub_seq_frame_num;
        }sub_seq_info;

        struct SubSeqLayerCharacteristics
        {
            mfxU8  num_sub_seq_layers;
            mfxU8  accurate_statistics_flag[16];
            mfxU16 average_bit_rate[16];
            mfxU16 average_frame_rate[16];
        }sub_seq_layer_characteristics;

        struct SubSeqCharacteristics
        {
            mfxU8  sub_seq_layer_num;
            mfxU8  sub_seq_id;
            mfxU8  duration_flag;
            mfxU8  sub_seq_duration;
            mfxU8  average_rate_flag;
            mfxU8  accurate_statistics_flag;
            mfxU16 average_bit_rate;
            mfxU16 average_frame_rate;
            mfxU8  num_referenced_subseqs;
            mfxU8  ref_sub_seq_layer_num[16];
            mfxU8  ref_sub_seq_id[16];
            mfxU8  ref_sub_seq_direction[16];
        }sub_seq_characteristics;

        struct FullFrameFreeze
        {
            mfxU32 full_frame_freeze_repetition_period;
        }full_frame_freeze;

        struct FullFrameSnapshot
        {
            mfxU8 snapshot_id;
        }full_frame_snapshot;

        struct ProgressiveRefinementSegmentStart
        {
            mfxU8 progressive_refinement_id;
            mfxU8 num_refinement_steps;
        }progressive_refinement_segment_start;

        struct MotionConstrainedSliceGroupSet
        {
            mfxU8 num_slice_groups_in_set;
            mfxU8 slice_group_id[8];
            mfxU8 exact_sample_value_match_flag;
            mfxU8 pan_scan_rect_flag;
            mfxU8 pan_scan_rect_id;
        }motion_constrained_slice_group_set;

        struct FilmGrainCharacteristics
        {
            mfxU8 film_grain_characteristics_cancel_flag;
            mfxU8 model_id;
            mfxU8 separate_colour_description_present_flag;
            mfxU8 film_grain_bit_depth_luma;
            mfxU8 film_grain_bit_depth_chroma;
            mfxU8 film_grain_full_range_flag;
            mfxU8 film_grain_colour_primaries;
            mfxU8 film_grain_transfer_characteristics;
            mfxU8 film_grain_matrix_coefficients;
            mfxU8 blending_mode_id;
            mfxU8 log2_scale_factor;
            mfxU8 comp_model_present_flag[3];
            mfxU8 num_intensity_intervals[3];
            mfxU8 num_model_values[3];
            mfxU8 intensity_interval_lower_bound[3][256];
            mfxU8 intensity_interval_upper_bound[3][256];
            mfxU8 comp_model_value[3][3][256];
            mfxU8 film_grain_characteristics_repetition_period;
        }film_grain_characteristics;

        struct DeblockingFilterDisplayPreference
        {
            mfxU8 deblocking_display_preference_cancel_flag;
            mfxU8 display_prior_to_deblocking_preferred_flag;
            mfxU8 dec_frame_buffering_constraint_flag;
            mfxU8 deblocking_display_preference_repetition_period;
        }deblocking_filter_display_preference;

        struct StereoVideoInfo
        {
            mfxU8 field_views_flag;
            mfxU8 top_field_is_left_view_flag;
            mfxU8 current_frame_is_left_view_flag;
            mfxU8 next_frame_is_second_view_flag;
            mfxU8 left_view_self_contained_flag;
            mfxU8 right_view_self_contained_flag;
        }stereo_video_info;

    }SEI_messages;

    void Reset()
    {
        memset(this, 0, sizeof(AVCSEIPayLoadBase));

        payLoadType = SEI_RESERVED;
        payLoadSize = 0;
    }
};

struct AVCSEIPayLoad : public HeapObject, public AVCSEIPayLoadBase
{
    std::vector<mfxU8> user_data; // for UserDataRegistered or UserDataUnRegistered

    AVCSEIPayLoad()
        : AVCSEIPayLoadBase()
    {
    }

    virtual void Reset()
    {
        AVCSEIPayLoadBase::Reset();
        user_data.clear();
    }

    mfxI32 GetID() const
    {
        return payLoadType;
    }
};

#pragma pack()

// This file defines some data structures and constants used by the decoder,
// that are also needed by other classes, such as post filters and
// error concealment.

#define INTERP_FACTOR 4
#define INTERP_SHIFT 2

#define CHROMA_INTERP_FACTOR 8
#define CHROMA_INTERP_SHIFT 3

// at picture edge, clip motion vectors to only this far beyond the edge,
// in pixel units.
#define D_MV_CLIP_LIMIT 19

enum Direction_t{
    D_DIR_FWD = 0,
    D_DIR_BWD = 1,
    D_DIR_BIDIR = 2,
    D_DIR_DIRECT = 3,
    D_DIR_DIRECT_SPATIAL_FWD = 4,
    D_DIR_DIRECT_SPATIAL_BWD = 5,
    D_DIR_DIRECT_SPATIAL_BIDIR = 6
};

inline bool IsForwardOnly(mfxI32 direction)
{
    return (direction == D_DIR_FWD) || (direction == D_DIR_DIRECT_SPATIAL_FWD);
}

inline bool IsHaveForward(mfxI32 direction)
{
    return (direction == D_DIR_FWD) || (direction == D_DIR_BIDIR) ||
        (direction == D_DIR_DIRECT_SPATIAL_FWD) || (direction == D_DIR_DIRECT_SPATIAL_BIDIR) ||
         (direction == D_DIR_DIRECT);
}

inline bool IsBackwardOnly(mfxI32 direction)
{
    return (direction == D_DIR_BWD) || (direction == D_DIR_DIRECT_SPATIAL_BWD);
}

inline bool IsHaveBackward(mfxI32 direction)
{
    return (direction == D_DIR_BWD) || (direction == D_DIR_BIDIR) ||
        (direction == D_DIR_DIRECT_SPATIAL_BWD) || (direction == D_DIR_DIRECT_SPATIAL_BIDIR) ||
        (direction == D_DIR_DIRECT);
}

inline bool IsBidirOnly(mfxI32 direction)
{
    return (direction == D_DIR_BIDIR) || (direction == D_DIR_DIRECT_SPATIAL_BIDIR) ||
        (direction == D_DIR_DIRECT);
}

// Warning: If these bit defines change, also need to change same
// defines  and related code in sresidual.s.
enum CBP
{
    D_CBP_LUMA_DC = 0x00001,
    D_CBP_LUMA_AC = 0x1fffe,

    D_CBP_CHROMA_DC = 0x00001,
    D_CBP_CHROMA_AC = 0x1fffe,
    D_CBP_CHROMA_AC_420 = 0x0001e,
    D_CBP_CHROMA_AC_422 = 0x001fe,
    D_CBP_CHROMA_AC_444 = 0x1fffe,

    D_CBP_1ST_LUMA_AC_BITPOS = 1,
    D_CBP_1ST_CHROMA_DC_BITPOS = 17,
    D_CBP_1ST_CHROMA_AC_BITPOS = 19
};

enum
{
    FIRST_DC_LUMA = 0,
    FIRST_AC_LUMA = 1,
    FIRST_DC_CHROMA = 17,
    FIRST_AC_CHROMA = 19
};

enum
{
    CHROMA_FORMAT_400       = 0,
    CHROMA_FORMAT_420       = 1,
    CHROMA_FORMAT_422       = 2,
    CHROMA_FORMAT_444       = 3
};

class AVC_exception
{
public:
    AVC_exception(mfxI32 status = -1)
        : m_Status(status)
    {
    }

    virtual ~AVC_exception()
    {
    }

    mfxI32 GetStatus() const
    {
        return m_Status;
    }

private:
    mfxI32 m_Status;
};


#pragma pack(1)

extern mfxI32 lock_failed;

#pragma pack()

template <typename T>
inline T * AVC_new_array_throw(mfxI32 size)
{
    T * t = new T[size];
    if (!t)
        throw AVC_exception(MFX_ERR_MEMORY_ALLOC);
    return t;
}

template <typename T>
inline T * AVC_new_throw()
{
    T * t = new T();
    if (!t)
        throw AVC_exception(MFX_ERR_MEMORY_ALLOC);
    return t;
}

template <typename T, typename T1>
inline T * AVC_new_throw_1(T1 t1)
{
    T * t = new T(t1);
    if (!t)
        throw AVC_exception(MFX_ERR_MEMORY_ALLOC);
    return t;
}

inline mfxU32 CalculateSuggestedSize(const AVCSeqParamSet * sps)
{
    mfxU32 base_size = sps->frame_width_in_mbs * sps->frame_height_in_mbs * 256;
    mfxU32 size = 0;

    switch (sps->chroma_format_idc)
    {
    case 0:  // YUV400
        size = base_size;
        break;
    case 1:  // YUV420
        size = (base_size * 3) / 2;
        break;
    case 2: // YUV422
        size = base_size + base_size;
        break;
    case 3: // YUV444
        size = base_size + base_size + base_size;
        break;
    };

    return size;
}

#define SAMPLE_ASSERT(x)

} // namespace ProtectedLibrary


#endif // __AVC_STRUCTURES_H__
