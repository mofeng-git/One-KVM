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


#include "brc_routines.h"
#include "math.h"
#include "mfxdefs.h"
#include <algorithm>

#ifndef MFX_VERSION
#error MFX_VERSION not defined
#endif

#if (MFX_VERSION >= 1024)
#define BRC_SCENE_CHANGE_RATIO1 20.0
#define BRC_SCENE_CHANGE_RATIO2 5.0

mfxExtBuffer* Hevc_GetExtBuffer(mfxExtBuffer** extBuf, mfxU32 numExtBuf, mfxU32 id)
{
    if (extBuf != 0)
    {
        for (mfxU16 i = 0; i < numExtBuf; i++)
        {
            if (extBuf[i] != 0 && extBuf[i]->BufferId == id) // assuming aligned buffers
                return (extBuf[i]);
        }
    }

    return nullptr;
}

mfxStatus cBRCParams::Init(mfxVideoParam* par, bool bFielMode)
{
    printf("Sample BRC is used\n");
    MFX_CHECK_NULL_PTR1(par);
    MFX_CHECK(par->mfx.RateControlMethod == MFX_RATECONTROL_CBR ||
              par->mfx.RateControlMethod == MFX_RATECONTROL_VBR,
              MFX_ERR_UNDEFINED_BEHAVIOR);

    mfxU32 k = par->mfx.BRCParamMultiplier == 0 ?  1: par->mfx.BRCParamMultiplier;
    mfxU32 bpsScale  = (par->mfx.CodecId == MFX_CODEC_AVC) ? 10 : 6;

    rateControlMethod  = par->mfx.RateControlMethod;
    targetbps = (((k*par->mfx.TargetKbps*1000) >> bpsScale) << bpsScale);
    maxbps    = (((k*par->mfx.MaxKbps   *1000) >> bpsScale) << bpsScale);

    maxbps = (par->mfx.RateControlMethod == MFX_RATECONTROL_CBR) ?
        targetbps : ((maxbps >= targetbps) ? maxbps : targetbps);
    mfxExtCodingOption * pExtCO = (mfxExtCodingOption*)Hevc_GetExtBuffer(par->ExtParam, par->NumExtParam, MFX_EXTBUFF_CODING_OPTION);

    HRDConformance = MFX_BRC_NO_HRD;
    if (pExtCO)
    {
        if ((MFX_CODINGOPTION_OFF != pExtCO->NalHrdConformance) && (MFX_CODINGOPTION_OFF != pExtCO->VuiNalHrdParameters))
            HRDConformance = MFX_BRC_HRD_STRONG;
        else if ((MFX_CODINGOPTION_ON == pExtCO->NalHrdConformance) && (MFX_CODINGOPTION_OFF == pExtCO->VuiNalHrdParameters))
            HRDConformance = MFX_BRC_HRD_WEAK;
    }

    if (HRDConformance != MFX_BRC_NO_HRD)
    {
        bufferSizeInBytes   = ((k*par->mfx.BufferSizeInKB  *1000) >> 3) << 3;
        initialDelayInBytes = ((k*par->mfx.InitialDelayInKB*1000) >> 3) << 3;
        bRec = 1;
        bPanic = (HRDConformance == MFX_BRC_HRD_STRONG) ? 1 : 0;
    }
    MFX_CHECK (par->mfx.FrameInfo.FrameRateExtD != 0 &&
               par->mfx.FrameInfo.FrameRateExtN != 0,
               MFX_ERR_UNDEFINED_BEHAVIOR);

    frameRate = (mfxF64)par->mfx.FrameInfo.FrameRateExtN / (mfxF64)par->mfx.FrameInfo.FrameRateExtD;

    width  = par->mfx.FrameInfo.Width;
    height = par->mfx.FrameInfo.Height;

    chromaFormat = par->mfx.FrameInfo.ChromaFormat == 0 ?  MFX_CHROMAFORMAT_YUV420 : par->mfx.FrameInfo.ChromaFormat ;
    bitDepthLuma = par->mfx.FrameInfo.BitDepthLuma == 0 ?  8 : par->mfx.FrameInfo.BitDepthLuma;
    quantOffset  = 6 * (bitDepthLuma - 8);

    inputBitsPerFrame    = targetbps / frameRate;
    maxInputBitsPerFrame = maxbps / frameRate;
    gopPicSize = par->mfx.GopPicSize*(bFielMode ? 2 : 1);
    gopRefDist = par->mfx.GopRefDist*(bFielMode ? 2 : 1);

    mfxExtCodingOption2 * pExtCO2 = (mfxExtCodingOption2*)Hevc_GetExtBuffer(par->ExtParam, par->NumExtParam, MFX_EXTBUFF_CODING_OPTION2);
    bPyr = (pExtCO2 && pExtCO2->BRefType == MFX_B_REF_PYRAMID);
    maxFrameSizeInBits  = pExtCO2 ? pExtCO2->MaxFrameSize*8 : 0;
    fAbPeriodLong  = 100;
    fAbPeriodShort = 5;
    dqAbPeriod = 100;
    bAbPeriod = 100;

    if (maxFrameSizeInBits)
    {
        bRec = 1;
        bPanic = 1;
    }

    if (pExtCO2
        && pExtCO2->MaxQPI <=51 && pExtCO2->MaxQPI > pExtCO2->MinQPI && pExtCO2->MinQPI >=1
        && pExtCO2->MaxQPP <=51 && pExtCO2->MaxQPP > pExtCO2->MinQPP && pExtCO2->MinQPP >=1
        && pExtCO2->MaxQPB <=51 && pExtCO2->MaxQPB > pExtCO2->MinQPB && pExtCO2->MinQPB >=1 )
    {
        quantMaxI = pExtCO2->MaxQPI + quantOffset;
        quantMinI = pExtCO2->MinQPI;
        quantMaxP = pExtCO2->MaxQPP + quantOffset;
        quantMinP = pExtCO2->MinQPP;
        quantMaxB = pExtCO2->MaxQPB + quantOffset;
        quantMinB = pExtCO2->MinQPB;
    }
    else
    {
        quantMaxI = quantMaxP = quantMaxB = 51 + quantOffset;
        quantMinI = quantMinP = quantMinB = 1;
    }

    mfxExtCodingOption3 * pExtCO3 = (mfxExtCodingOption3*)Hevc_GetExtBuffer(par->ExtParam, par->NumExtParam, MFX_EXTBUFF_CODING_OPTION3);
    if (pExtCO3)
    {
        WinBRCMaxAvgKbps = pExtCO3->WinBRCMaxAvgKbps*par->mfx.BRCParamMultiplier;
        WinBRCSize = pExtCO3->WinBRCSize;
    }
    return MFX_ERR_NONE;
}

mfxStatus   cBRCParams::GetBRCResetType(mfxVideoParam* par, bool bNewSequence, bool &bBRCReset, bool &bSlidingWindowReset)
{
    bBRCReset = false;
    bSlidingWindowReset = false;

    if (bNewSequence)
        return MFX_ERR_NONE;

    cBRCParams new_par;
    mfxStatus sts = new_par.Init(par);
    MFX_CHECK_STS(sts);

    MFX_CHECK(new_par.rateControlMethod == rateControlMethod, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM) ;
    MFX_CHECK(new_par.HRDConformance == HRDConformance, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM) ;
    MFX_CHECK(new_par.frameRate == frameRate, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM);
    MFX_CHECK(new_par.width == width, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM);
    MFX_CHECK(new_par.height == height, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM);
    MFX_CHECK(new_par.chromaFormat == chromaFormat, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM);
    MFX_CHECK(new_par.bitDepthLuma == bitDepthLuma, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM);

    if (HRDConformance == MFX_BRC_HRD_STRONG)
    {
        MFX_CHECK(new_par.bufferSizeInBytes   == bufferSizeInBytes, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM);
        MFX_CHECK(new_par.initialDelayInBytes == initialDelayInBytes, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM);
        MFX_CHECK(new_par.targetbps == targetbps, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM);
        MFX_CHECK(new_par.maxbps == maxbps, MFX_ERR_INCOMPATIBLE_VIDEO_PARAM);
    }
    else if (new_par.targetbps != targetbps || new_par.maxbps != maxbps)
    {
        bBRCReset = true;
    }

    if (new_par.WinBRCMaxAvgKbps != WinBRCMaxAvgKbps)
    {
        bBRCReset = true;
        bSlidingWindowReset = true;
    }

    if (new_par.maxFrameSizeInBits != maxFrameSizeInBits) bBRCReset = true;
    if (new_par.gopPicSize != gopPicSize) bBRCReset = true;
    if (new_par.gopRefDist != gopRefDist) bBRCReset = true;
    if (new_par.bPyr != bPyr) bBRCReset = true;
    if (new_par.quantMaxI != quantMaxI) bBRCReset = true;
    if (new_par.quantMinI != quantMinI) bBRCReset = true;
    if (new_par.quantMaxP != quantMaxP) bBRCReset = true;
    if (new_par.quantMinP != quantMinP) bBRCReset = true;
    if (new_par.quantMaxB != quantMaxB) bBRCReset = true;
    if (new_par.quantMinB != quantMinB) bBRCReset = true;

    return MFX_ERR_NONE;
}



enum
{
    MFX_BRC_RECODE_NONE           = 0,
    MFX_BRC_RECODE_QP             = 1,
    MFX_BRC_RECODE_PANIC          = 2,
};

   mfxF64 const QSTEP[88] = {
         0.630,  0.707,  0.794,  0.891,  1.000,   1.122,   1.260,   1.414,   1.587,   1.782,   2.000,   2.245,   2.520,
         2.828,  3.175,  3.564,  4.000,  4.490,   5.040,   5.657,   6.350,   7.127,   8.000,   8.980,  10.079,  11.314,
        12.699, 14.254, 16.000, 17.959, 20.159,  22.627,  25.398,  28.509,  32.000,  35.919,  40.317,  45.255,  50.797,
        57.018, 64.000, 71.838, 80.635, 90.510, 101.594, 114.035, 128.000, 143.675, 161.270, 181.019, 203.187, 228.070,
        256.000, 287.350, 322.540, 362.039, 406.375, 456.140, 512.000, 574.701, 645.080, 724.077, 812.749, 912.280,
        1024.000, 1149.401, 1290.159, 1448.155, 1625.499, 1824.561, 2048.000, 2298.802, 2580.318, 2896.309, 3250.997, 3649.121,
        4096.000, 4597.605, 5160.637, 5792.619, 6501.995, 7298.242, 8192.000, 9195.209, 10321.273, 11585.238, 13003.989, 14596.485
    };


mfxI32 QStep2QpFloor(mfxF64 qstep, mfxI32 qpoffset = 0) // QSTEP[qp] <= qstep, return 0<=qp<=51+mQuantOffset
{
    mfxU8 qp = mfxU8(std::upper_bound(QSTEP, QSTEP + 51 + qpoffset, qstep) - QSTEP);
    return qp > 0 ? qp - 1 : 0;
}

mfxI32 Qstep2QP(mfxF64 qstep, mfxI32 qpoffset = 0) // return 0<=qp<=51+mQuantOffset
{
    mfxI32 qp = QStep2QpFloor(qstep, qpoffset);

    // prevent going QSTEP index out of bounds
    if (qp >= (mfxI32)(sizeof(QSTEP)/sizeof(mfxF64)) - 1)
        return 0;
    return (qp == 51 + qpoffset || qstep < (QSTEP[qp] + QSTEP[qp + 1]) / 2) ? qp : qp + 1;
}
mfxF64 QP2Qstep(mfxI32 qp, mfxI32 qpoffset = 0)
{
    return QSTEP[std::min(51 + qpoffset, qp)];
}


mfxF64  cHRD::GetBufferDiviation(mfxU32 targetBitrate)
{
    mfxI64 targetFullness    = std::min(m_delayInBits, m_buffSizeInBits / 2);
    mfxI64 minTargetFullness = std::min<mfxU32>(m_buffSizeInBits / 2, targetBitrate * 2); // half bufsize or 2 sec
    targetFullness           = std::max(targetFullness, minTargetFullness);

    return targetFullness - m_bufFullness;
}

mfxU16 cHRD::UpdateAndCheckHRD(mfxI32 frameBits, mfxI32 recode, mfxI32 minQuant, mfxI32 maxQuant)
{
    mfxU16 brcStatus = MFX_BRC_OK ;

    if (recode  == 0)
    {
        m_prevBufFullness = m_bufFullness;
        m_underflowQuant = minQuant - 1;
        m_overflowQuant  = maxQuant + 1;
    }
    else
    { // frame is being recoded - restore buffer state
        m_bufFullness = m_prevBufFullness;
        m_frameNum--;
    }

    m_maxFrameSize = (mfxI32)(m_bufFullness - 1);
    m_minFrameSize = (!m_bCBR)? 0 : (mfxI32)(m_bufFullness + 1 + 1 + m_inputBitsPerFrame - m_buffSizeInBits);
    if (m_minFrameSize < 0)
        m_minFrameSize = 0;

   mfxF64  bufFullness = m_bufFullness - frameBits;

    if (bufFullness < 2)
    {
        bufFullness = m_inputBitsPerFrame;
        brcStatus = MFX_BRC_BIG_FRAME;
        if (bufFullness > m_buffSizeInBits)
            bufFullness = m_buffSizeInBits;
    }
    else
    {
        bufFullness += m_inputBitsPerFrame;
        if (bufFullness > m_buffSizeInBits - 1)
        {
            bufFullness = m_buffSizeInBits - 1;
            if (m_bCBR)
                brcStatus = MFX_BRC_SMALL_FRAME;
        }
    }
    m_frameNum++;
    if ( MFX_BRC_RECODE_PANIC == recode) // no use in changing QP
    {
        if (brcStatus == MFX_BRC_SMALL_FRAME)
            brcStatus =  MFX_BRC_PANIC_SMALL_FRAME ;
        if (brcStatus == MFX_BRC_BIG_FRAME)
            brcStatus =  MFX_BRC_PANIC_BIG_FRAME ;    }

    m_bufFullness = bufFullness;
    return brcStatus;
}

mfxStatus cHRD::UpdateMinMaxQPForRec( mfxU32 brcSts, mfxI32 qp)
{
    MFX_CHECK(brcSts == MFX_BRC_BIG_FRAME || brcSts == MFX_BRC_SMALL_FRAME, MFX_ERR_UNDEFINED_BEHAVIOR);
    if (brcSts == MFX_BRC_BIG_FRAME)
        m_underflowQuant = qp;
    else
        m_overflowQuant = qp;
    return MFX_ERR_NONE;
}
mfxI32 cHRD::GetTargetSize(mfxU32 brcSts)
{
     if (brcSts != MFX_BRC_BIG_FRAME && brcSts != MFX_BRC_SMALL_FRAME) return 0;
     return (brcSts == MFX_BRC_BIG_FRAME) ? m_maxFrameSize * 3 / 4 : m_minFrameSize * 5 / 4;
}

mfxI32 GetNewQP(mfxF64 totalFrameBits, mfxF64 targetFrameSizeInBits, mfxI32 minQP , mfxI32 maxQP, mfxI32 qp , mfxI32 qp_offset, mfxF64 f_pow, bool bStrict = false, bool bLim = true)
{
    mfxF64 qstep = 0, qstep_new = 0;
    mfxI32 qp_new = qp;

    qstep = QP2Qstep(qp, qp_offset);
    qstep_new = qstep * pow(totalFrameBits / targetFrameSizeInBits, f_pow);
    qp_new = Qstep2QP(qstep_new, qp_offset);

    if (totalFrameBits < targetFrameSizeInBits) // overflow
    {
        if (qp <= minQP)
        {
            return qp; // QP change is impossible
        }
        if (bLim)
            qp_new  = std::max(qp_new , (minQP + qp + 1) >> 1);
        if (bStrict)
            qp_new  = std::min(qp_new, qp - 1);
    }
    else // underflow
    {
        if (qp >= maxQP)
        {
            return qp; // QP change is impossible
        }
        if (bLim)
            qp_new  = std::min(qp_new , (maxQP + qp + 1) >> 1);
        if (bStrict)
            qp_new  = std::max(qp_new, qp + 1);
    }
    return mfx::clamp(qp_new, minQP, maxQP);
}




void cHRD::Init(mfxU32 buffSizeInBytes, mfxU32 delayInBytes, mfxF64 inputBitsPerFrame, bool bCBR)
{
    m_bufFullness = m_prevBufFullness= delayInBytes << 3;
    m_delayInBits = delayInBytes << 3;
    m_buffSizeInBits = buffSizeInBytes << 3;
    m_inputBitsPerFrame =inputBitsPerFrame;
    m_bCBR = bCBR;

    m_underflowQuant = 0;
    m_overflowQuant = 999;
    m_frameNum = 0;
    m_minFrameSize = 0;
    m_maxFrameSize = 0;

}

void UpdateQPParams(mfxI32 qp, mfxU32 type , BRC_Ctx  &ctx, mfxU32 /* rec_num */, mfxI32 minQuant, mfxI32 maxQuant, mfxU32 level)
{
    ctx.Quant = qp;
    if (type == MFX_FRAMETYPE_I)
    {
        ctx.QuantI = qp;
        ctx.QuantP = qp + 1;
        ctx.QuantB = qp + 2;
    }
    else if (type == MFX_FRAMETYPE_P)
    {
        qp -= level;
        ctx.QuantI =  qp - 1;
        ctx.QuantP = qp;
        ctx.QuantB = qp + 1;
    }
    else if (type == MFX_FRAMETYPE_B)
    {
        level = level > 0 ? level - 1: 0;
        qp -= level;
        ctx.QuantI = qp - 2;
        ctx.QuantP = qp - 1;
        ctx.QuantB = qp;
    }
    ctx.QuantI = mfx::clamp(ctx.QuantI, minQuant, maxQuant);
    ctx.QuantP = mfx::clamp(ctx.QuantP, minQuant, maxQuant);
    ctx.QuantB = mfx::clamp(ctx.QuantB, minQuant, maxQuant);
    //printf("ctx.QuantI %d, ctx.QuantP %d, ctx.QuantB  %d, level %d\n", ctx.QuantI, ctx.QuantP, ctx.QuantB, level);
}

mfxI32 GetRawFrameSize(mfxU32 lumaSize, mfxU16 chromaFormat, mfxU16 bitDepthLuma)
{
    mfxI32 frameSize = lumaSize;

  if (chromaFormat == MFX_CHROMAFORMAT_YUV420)
    frameSize += lumaSize / 2;
  else if (chromaFormat == MFX_CHROMAFORMAT_YUV422)
   frameSize += lumaSize;
  else if (chromaFormat == MFX_CHROMAFORMAT_YUV444)
    frameSize += lumaSize * 2;

  frameSize = frameSize * bitDepthLuma / 8;
  return frameSize*8; //frame size in bits
}

mfxStatus ExtBRC::Init (mfxVideoParam* par)
{
    mfxStatus sts = MFX_ERR_NONE;

    MFX_CHECK(!m_bInit, MFX_ERR_UNDEFINED_BEHAVIOR);
    sts = m_par.Init(par);
    MFX_CHECK_STS(sts);

    if (m_par.HRDConformance != MFX_BRC_NO_HRD)
    {
        m_hrd.Init(m_par.bufferSizeInBytes, m_par.initialDelayInBytes, m_par.maxInputBitsPerFrame, m_par.rateControlMethod == MFX_RATECONTROL_CBR);
    }
    memset(&m_ctx, 0, sizeof(m_ctx));

    m_ctx.fAbLong  = m_par.inputBitsPerFrame;
    m_ctx.fAbShort = m_par.inputBitsPerFrame;
    m_ctx.encOrder = mfxU32(-1);

    mfxI32 rawSize = GetRawFrameSize(m_par.width * m_par.height ,m_par.chromaFormat, m_par.bitDepthLuma);
    mfxI32 qp = GetNewQP(rawSize, m_par.inputBitsPerFrame, m_par.quantMinI, m_par.quantMaxI, 1 , m_par.quantOffset, 0.5, false, false);

    UpdateQPParams(qp,MFX_FRAMETYPE_I , m_ctx, 0, m_par.quantMinI, m_par.quantMaxI, 0);

    m_ctx.dQuantAb = qp > 0 ? 1./qp : 1.0; //kw

    if (m_par.WinBRCSize)
    {
        m_avg.reset(new AVGBitrate(m_par.WinBRCSize, (mfxU32)(m_par.WinBRCMaxAvgKbps*1000.0/m_par.frameRate), (mfxU32)m_par.inputBitsPerFrame) );
        MFX_CHECK_NULL_PTR1(m_avg.get());
    }

    m_bInit = true;
    return sts;
}

mfxU16 GetFrameType(mfxU16 m_frameType, mfxU16 level, mfxU16 gopRegDist)
{
    if (m_frameType & MFX_FRAMETYPE_IDR)
        return MFX_FRAMETYPE_I;
    else if (m_frameType & MFX_FRAMETYPE_I)
        return MFX_FRAMETYPE_I;
    else if (m_frameType & MFX_FRAMETYPE_P)
        return MFX_FRAMETYPE_P;
    else if ((m_frameType & MFX_FRAMETYPE_REF) && (level == 0 || gopRegDist == 1))
        return MFX_FRAMETYPE_P; //low delay B
    else
        return MFX_FRAMETYPE_B;
}


bool  isFrameBeforeIntra (mfxU32 order, mfxU32 intraOrder, mfxU32 gopPicSize, mfxU32 gopRefDist)
{
    mfxI32 distance0 = gopPicSize*3/4;
    mfxI32 distance1 = gopPicSize - gopRefDist*3;
    return mfxI32(order - intraOrder) > std::max(distance0, distance1);
}

mfxStatus SetRecodeParams(mfxU16 brcStatus, mfxI32 qp, mfxI32 qp_new, mfxI32 minQP, mfxI32 maxQP, BRC_Ctx &ctx, mfxBRCFrameStatus* status)
{
    ctx.bToRecode = 1;

    if (brcStatus == MFX_BRC_BIG_FRAME || brcStatus == MFX_BRC_PANIC_BIG_FRAME )
    {
         MFX_CHECK(qp_new >= qp, MFX_ERR_UNDEFINED_BEHAVIOR);
         ctx.Quant = qp_new;
         ctx.QuantMax = maxQP;
         if (brcStatus == MFX_BRC_BIG_FRAME && qp_new > qp)
         {
            ctx.QuantMin = std::max(qp + 1, minQP); //limit QP range for recoding
            status->BRCStatus = MFX_BRC_BIG_FRAME;

         }
         else
         {
             ctx.QuantMin = minQP;
             ctx.bPanic = 1;
             status->BRCStatus = MFX_BRC_PANIC_BIG_FRAME;
         }
    }
    else if (brcStatus == MFX_BRC_SMALL_FRAME || brcStatus == MFX_BRC_PANIC_SMALL_FRAME)
    {
         MFX_CHECK(qp_new <= qp, MFX_ERR_UNDEFINED_BEHAVIOR);

         ctx.Quant = qp_new;
         ctx.QuantMin = minQP; //limit QP range for recoding

         if (brcStatus == MFX_BRC_SMALL_FRAME && qp_new < qp)
         {
            ctx.QuantMax = std::min(qp - 1, maxQP);
            status->BRCStatus = MFX_BRC_SMALL_FRAME;
         }
         else
         {
            ctx.QuantMax = maxQP;
            status->BRCStatus = MFX_BRC_PANIC_SMALL_FRAME;
            ctx.bPanic = 1;
         }
    }
    //printf("recode %d , qp %d new %d, status %d\n", ctx.encOrder, qp, qp_new, status->BRCStatus);
    return MFX_ERR_NONE;
}
mfxI32 GetNewQPTotal(mfxF64 bo, mfxF64 dQP, mfxI32 minQP , mfxI32 maxQP, mfxI32 qp, bool bPyr, bool bSC)
{
    mfxU8 mode = (!bPyr) ;

    bo  = mfx::clamp(bo, -1.0, 1.0);
    dQP = mfx::clamp(dQP, 1./maxQP, 1./minQP);
    dQP = dQP + (1./maxQP - dQP) * bo;
    dQP = mfx::clamp(dQP, 1./maxQP, 1./minQP);
    mfxI32 quant_new = (mfxI32) (1. / dQP + 0.5);

    //printf("   GetNewQPTotal: bo %f, quant %d, quant_new %d, mode %d\n", bo, qp, quant_new, mode);
    if (!bSC)
    {
        if (mode == 0) // low: qp_diff [-2; 2]
        {
            if (quant_new >= qp + 5)
                quant_new = qp + 2;
            else if (quant_new > qp + 3)
                quant_new = qp + 1;
            else if (quant_new <= qp - 5)
                quant_new = qp - 2;
            else if (quant_new < qp - 2)
                quant_new = qp - 1;
        }
        else // (mode == 1) midle: qp_diff [-3; 3]
        {
            if (quant_new >= qp + 5)
                quant_new = qp + 3;
            else if (quant_new > qp + 3)
                quant_new = qp + 2;
            else if (quant_new <= qp - 5)
                quant_new = qp - 3;
            else if (quant_new < qp - 2)
                quant_new = qp - 2;
        }

    }
    else
    {
        quant_new = mfx::clamp(quant_new, qp - 5, qp + 5);
    }
    return mfx::clamp(quant_new, minQP, maxQP);
}
// Reduce AB period before intra and increase it after intra (to avoid intra frame affect on the bottom of hrd)
mfxF64 GetAbPeriodCoeff (mfxU32 numInGop, mfxU32 gopPicSize)
{
    const mfxU32 maxForCorrection = 30;
    const mfxF64 maxValue = 1.5;
    const mfxF64 minValue = 1.0;

    mfxU32 numForCorrection = std::min(gopPicSize /2, maxForCorrection);
    mfxF64 k[maxForCorrection] = {};

    if (numInGop >= gopPicSize || gopPicSize < 2)
        return 1.0;

    for (mfxU32 i = 0; i < numForCorrection; i ++)
    {
        k[i] = maxValue - (maxValue - minValue)*i/numForCorrection;
    }
    if (numInGop < gopPicSize/2)
    {
        return k [numInGop < numForCorrection ? numInGop : numForCorrection - 1];
    }
    else
    {
        mfxU32 n = gopPicSize - 1 - numInGop;
        return 1.0/ k[n < numForCorrection ? n : numForCorrection - 1];
    }

}

mfxI32 ExtBRC::GetCurQP (mfxU32 type, mfxI32 layer)
{
    mfxI32 qp = 0;
    if (type == MFX_FRAMETYPE_I)
    {
        qp = m_ctx.QuantI;
        qp = mfx::clamp(qp, m_par.quantMinI, m_par.quantMaxI);
    }
    else if (type == MFX_FRAMETYPE_P)
    {
        qp = m_ctx.QuantP + layer;
        qp = mfx::clamp(qp, m_par.quantMinP, m_par.quantMaxP);
    }
    else
    {
        qp = m_ctx.QuantB + (layer > 0 ? layer - 1 : 0);
        qp = mfx::clamp(qp, m_par.quantMinB, m_par.quantMaxB);
    }
    //printf("GetCurQP I %d P %d B %d, min %d max %d type %d \n", m_ctx.QuantI, m_ctx.QuantP, m_ctx.QuantB, m_par.quantMinI, m_par.quantMaxI, type);

    return qp;
}

mfxStatus ExtBRC::Update(mfxBRCFrameParam* frame_par, mfxBRCFrameCtrl* frame_ctrl, mfxBRCFrameStatus* status)
{
    mfxStatus sts       = MFX_ERR_NONE;

    MFX_CHECK_NULL_PTR3(frame_par, frame_ctrl, status);
    MFX_CHECK(m_bInit, MFX_ERR_NOT_INITIALIZED);

    mfxU16 &brcSts       = status->BRCStatus;
    status->MinFrameSize  = 0;

    //printf("ExtBRC::Update:  m_ctx.encOrder %d , frame_par->EncodedOrder %d, frame_par->NumRecode %d, frame_par->CodedFrameSize %d, qp %d\n", m_ctx.encOrder , frame_par->EncodedOrder, frame_par->NumRecode, frame_par->CodedFrameSize, frame_ctrl->QpY);

    mfxI32 bitsEncoded  = frame_par->CodedFrameSize*8;
    mfxU32 picType      = GetFrameType(frame_par->FrameType, frame_par->PyramidLayer, m_par.gopRefDist);
    mfxI32 qpY          = frame_ctrl->QpY + m_par.quantOffset;
    mfxI32 layer        = frame_par->PyramidLayer;
    mfxF64 qstep        = QP2Qstep(qpY, m_par.quantOffset);

    mfxF64 fAbLong  = m_ctx.fAbLong   + (bitsEncoded - m_ctx.fAbLong)  / m_par.fAbPeriodLong;
    mfxF64 fAbShort = m_ctx.fAbShort  + (bitsEncoded - m_ctx.fAbShort) / m_par.fAbPeriodShort;
    mfxF64 eRate    = bitsEncoded * sqrt(qstep);
    mfxF64 e2pe     =  0;
    bool bMaxFrameSizeMode = m_par.maxFrameSizeInBits != 0 &&
        m_par.rateControlMethod == MFX_RATECONTROL_VBR &&
        m_par.maxFrameSizeInBits < m_par.inputBitsPerFrame * 2 &&
        m_ctx.totalDiviation < (-1)*m_par.inputBitsPerFrame*m_par.frameRate;

    if (picType == MFX_FRAMETYPE_I)
        e2pe = (m_ctx.eRateSH == 0) ? (BRC_SCENE_CHANGE_RATIO2 + 1) : eRate / m_ctx.eRateSH;
    else
        e2pe = (m_ctx.eRate == 0) ? (BRC_SCENE_CHANGE_RATIO2 + 1) : eRate / m_ctx.eRate;

    mfxU32 frameSizeLim    = 0xfffffff ; // sliding window limitation or external frame size limitation

    bool  bSHStart = false;
    bool  bNeedUpdateQP = false;

    brcSts = MFX_BRC_OK;

    if (m_par.bRec && m_ctx.bToRecode &&  (m_ctx.encOrder != frame_par->EncodedOrder || frame_par->NumRecode == 0))
    {
        //printf("++++++++++++++++++++++++++++++++++\n");
        // Frame must be recoded, but encoder calls BR for another frame
        return MFX_ERR_UNDEFINED_BEHAVIOR;
    }
    if (frame_par->NumRecode == 0 || m_ctx.encOrder != frame_par->EncodedOrder)
    {
        // Set context for new frame
        if (picType == MFX_FRAMETYPE_I)
            m_ctx.LastIEncOrder = frame_par->EncodedOrder;
        m_ctx.encOrder = frame_par->EncodedOrder;
        m_ctx.poc = frame_par->DisplayOrder;
        m_ctx.bToRecode = 0;
        m_ctx.bPanic = 0;

        if (picType == MFX_FRAMETYPE_I)
        {
            m_ctx.QuantMin = m_par.quantMinI;
            m_ctx.QuantMax = m_par.quantMaxI;
        }
        else if (picType == MFX_FRAMETYPE_P)
        {
            m_ctx.QuantMin = m_par.quantMinP;
            m_ctx.QuantMax = m_par.quantMaxP;
        }
        else
        {
            m_ctx.QuantMin = m_par.quantMinB;
            m_ctx.QuantMax = m_par.quantMaxB;
        }
        m_ctx.Quant = qpY;


        if (m_ctx.SceneChange && ( m_ctx.poc > m_ctx.SChPoc + 1 || m_ctx.poc == 0))
            m_ctx.SceneChange &= ~16;

        bNeedUpdateQP = true;

        //printf("m_ctx.SceneChange %d, m_ctx.poc %d, m_ctx.SChPoc %d, m_ctx.poc %d \n", m_ctx.SceneChange, m_ctx.poc, m_ctx.SChPoc, m_ctx.poc);
    }
    if (e2pe > BRC_SCENE_CHANGE_RATIO2 )
    {
      // scene change, resetting BRC statistics
        fAbLong  = m_ctx.fAbLong  = m_par.inputBitsPerFrame;
        fAbShort = m_ctx.fAbShort = m_par.inputBitsPerFrame;
        m_ctx.SceneChange |= 1;
        if (picType != MFX_FRAMETYPE_B)
        {
            bSHStart = true;
            m_ctx.SceneChange |= 16;
            m_ctx.eRateSH = eRate;
            //if ((frame_par->DisplayOrder - m_ctx.SChPoc) >= std::min<mfxU32>(m_par.frameRate, m_par.gopRefDist))
            {
                m_ctx.dQuantAb  = 1./m_ctx.Quant;
            }
            m_ctx.SChPoc = frame_par->DisplayOrder;
            //printf("!!!!!!!!!!!!!!!!!!!!! %d m_ctx.SceneChange %d, order %d\n", frame_par->EncodedOrder, m_ctx.SceneChange, frame_par->DisplayOrder);
        }

    }
    if (m_par.HRDConformance != MFX_BRC_NO_HRD)
    {
        //check hrd
        brcSts = m_hrd.UpdateAndCheckHRD(bitsEncoded,frame_par->NumRecode, m_ctx.QuantMin,m_ctx.QuantMax);
        //printf("--UpdateAndCheckHRD (%d) brcSts %d, panic %d\n", frame_par->EncodedOrder,brcSts, m_ctx.bPanic);
        MFX_CHECK(brcSts ==  MFX_BRC_OK || (!m_ctx.bPanic), MFX_ERR_NOT_ENOUGH_BUFFER);
        if (brcSts == MFX_BRC_BIG_FRAME || brcSts == MFX_BRC_SMALL_FRAME)
            m_hrd.UpdateMinMaxQPForRec(brcSts, qpY);
        else
            bNeedUpdateQP = true;
        status->MinFrameSize = m_hrd.GetMinFrameSize();
        //printf("%d: poc %d, size %d QP %d (%d %d), HRD sts %d, maxFrameSize %d, type %d \n",frame_par->EncodedOrder, frame_par->DisplayOrder, bitsEncoded, m_ctx.Quant, m_ctx.QuantMin, m_ctx.QuantMax, brcSts,  m_hrd.GetMaxFrameSize(), frame_par->FrameType);
    }
    if (m_avg.get())
    {
       frameSizeLim = std::min(frameSizeLim, m_avg->GetMaxFrameSize(m_ctx.bPanic, bSHStart || picType == MFX_FRAMETYPE_I, frame_par->NumRecode));
    }
    if (m_par.maxFrameSizeInBits)
    {
        frameSizeLim = std::min(frameSizeLim, m_par.maxFrameSizeInBits);
    }
    //printf("frameSizeLim %d (%d)\n", frameSizeLim, bitsEncoded);

    if (frame_par->NumRecode < 2)
    // Check other condions for recoding (update qp is it is needed)
    {
        mfxF64 targetFrameSize = std::max<mfxF64>(m_par.inputBitsPerFrame, fAbLong);
        mfxF64 maxFrameSize = (m_ctx.encOrder == 0 ? 6.0 : (bSHStart || picType == MFX_FRAMETYPE_I) ? 8.0 : 4.0) * targetFrameSize*(m_par.bPyr ? 1.5 : 1.0) ;
        mfxI32 quantMax = m_ctx.QuantMax;
        mfxI32 quantMin = m_ctx.QuantMin;
        mfxI32 quant = qpY;

        maxFrameSize = std::min<mfxF64>(maxFrameSize, frameSizeLim);

        if (m_par.HRDConformance != MFX_BRC_NO_HRD)
        {
            if (bSHStart || picType == MFX_FRAMETYPE_I)
                maxFrameSize = std::min(maxFrameSize, 3.5/9. * m_hrd.GetMaxFrameSize() + 5.5/9. * targetFrameSize);
            else
                maxFrameSize = std::min(maxFrameSize, 2.5/9. * m_hrd.GetMaxFrameSize() + 6.5/9. * targetFrameSize);

            quantMax = std::min(m_hrd.GetMaxQuant(), quantMax);
            quantMin = std::max(m_hrd.GetMinQuant(), quantMin);
        }
        maxFrameSize = std::max(maxFrameSize, targetFrameSize);

        if (bitsEncoded >  maxFrameSize && quant < quantMax)
        {

            mfxI32 quant_new = GetNewQP(bitsEncoded, (mfxU32)maxFrameSize, quantMin , quantMax, quant ,m_par.quantOffset, 1);
            if (quant_new > quant )
            {
                bNeedUpdateQP = false;
                //printf("    recode 1-0: %d:  k %5f bitsEncoded %d maxFrameSize %d, targetSize %d, fAbLong %f, inputBitsPerFrame %f, qp %d new %d\n",frame_par->EncodedOrder, bitsEncoded/maxFrameSize, (int)bitsEncoded, (int)maxFrameSize,(int)targetFrameSize, fAbLong, m_par.inputBitsPerFrame, quant, quant_new);
                if (quant_new > GetCurQP (picType, layer))
                {
                    UpdateQPParams(bMaxFrameSizeMode? quant_new - 1 : quant_new ,picType, m_ctx, 0, quantMin , quantMax, layer);
                    fAbLong  =  m_ctx.fAbLong  = m_par.inputBitsPerFrame;
                    fAbShort =  m_ctx.fAbShort = m_par.inputBitsPerFrame;
                    m_ctx.dQuantAb = 1./quant_new;
                }

                if (m_par.bRec)
                {
                    SetRecodeParams(MFX_BRC_BIG_FRAME,quant,quant_new, quantMin, quantMax, m_ctx, status);
                    return sts;
                }
            } //(quant_new > quant)
        } //bitsEncoded >  maxFrameSize

        if (bitsEncoded >  maxFrameSize && quant == quantMax &&
            picType != MFX_FRAMETYPE_I && m_par.bPanic &&
            (!m_ctx.bPanic) && isFrameBeforeIntra(m_ctx.encOrder, m_ctx.LastIEncOrder, m_par.gopPicSize, m_par.gopRefDist))
        {
            //skip frames before intra
            SetRecodeParams(MFX_BRC_PANIC_BIG_FRAME,quant,quant, quantMin ,quantMax, m_ctx, status);
            return sts;
        }
        if (m_par.HRDConformance != MFX_BRC_NO_HRD && frame_par->NumRecode == 0 && (quant < quantMax))
        {
            mfxF64 FAMax = 1./9. * m_hrd.GetMaxFrameSize() + 8./9. * fAbLong;

            if (fAbShort > FAMax)
            {
                mfxI32 quant_new = GetNewQP(fAbShort, FAMax, quantMin , quantMax, quant ,m_par.quantOffset, 0.5);
                //printf("    recode 2-0: %d:  FAMax %f, fAbShort %f, quant_new %d\n",frame_par->EncodedOrder, FAMax, fAbShort, quant_new);

                if (quant_new > quant)
                {
                   bNeedUpdateQP = false;
                   if (quant_new > GetCurQP (picType, layer))
                   {
                        UpdateQPParams(quant_new ,picType, m_ctx, 0, quantMin , quantMax, layer);
                        fAbLong  = m_ctx.fAbLong  = m_par.inputBitsPerFrame;
                        fAbShort = m_ctx.fAbShort = m_par.inputBitsPerFrame;
                        m_ctx.dQuantAb = 1./quant_new;
                    }
                    if (m_par.bRec)
                    {
                        SetRecodeParams(MFX_BRC_BIG_FRAME,quant,quant_new, quantMin, quantMax, m_ctx, status);
                        return sts;
                    }
                }//quant_new > quant
            }
        }//m_par.HRDConformance
    }
    if (((m_par.HRDConformance != MFX_BRC_NO_HRD && brcSts != MFX_BRC_OK) || (bitsEncoded > (mfxI32)frameSizeLim)) && m_par.bRec)
    {
        mfxI32 quant = m_ctx.Quant;
        mfxI32 quant_new = quant;
        if (bitsEncoded > (mfxI32)frameSizeLim)
        {
            brcSts = MFX_BRC_BIG_FRAME;
            quant_new = GetNewQP(bitsEncoded, frameSizeLim, m_ctx.QuantMin , m_ctx.QuantMax,quant,m_par.quantOffset, 1, true);
        }
        else if (brcSts == MFX_BRC_BIG_FRAME || brcSts == MFX_BRC_SMALL_FRAME)
        {
            quant_new = GetNewQP(bitsEncoded, m_hrd.GetTargetSize(brcSts), m_ctx.QuantMin , m_ctx.QuantMax,quant,m_par.quantOffset, 1, true);
        }
        if (quant_new != quant)
        {
            if (brcSts == MFX_BRC_SMALL_FRAME)
            {
               quant_new = std::max(quant_new, quant-2);
               brcSts = MFX_BRC_PANIC_SMALL_FRAME;
            }
            // Idea is to check a sign mismatch, 'true' if both are negative or positive
            if ((quant_new - qpY) * (quant_new - GetCurQP (picType, layer)) > 0)
            {
                UpdateQPParams(quant_new ,picType, m_ctx, 0, m_ctx.QuantMin , m_ctx.QuantMax, layer);
            }
            bNeedUpdateQP = false;
        }
        SetRecodeParams(brcSts,quant,quant_new, m_ctx.QuantMin , m_ctx.QuantMax, m_ctx, status);
    }
    else
    {
        // no recoding are needed. Save context params

        mfxF64 k = 1./m_ctx.Quant;
        mfxF64 dqAbPeriod = m_par.dqAbPeriod;
        if (m_ctx.bToRecode)
            dqAbPeriod = (k < m_ctx.dQuantAb)? 16:25;

        if (bNeedUpdateQP)
        {
            m_ctx.dQuantAb += (k - m_ctx.dQuantAb)/dqAbPeriod;
            m_ctx.dQuantAb = mfx::clamp(m_ctx.dQuantAb, 1./m_ctx.QuantMax , 1./m_ctx.QuantMin);

            m_ctx.fAbLong  = fAbLong;
            m_ctx.fAbShort = fAbShort;
        }

        bool oldScene = false;
        if ((m_ctx.SceneChange & 16) && (m_ctx.poc < m_ctx.SChPoc) && (e2pe < .01) && (mfxF64)bitsEncoded < 1.5*fAbLong)
            oldScene = true;
        //printf("-- m_ctx.eRate %f,  eRate %f, e2pe %f\n", m_ctx.eRate,  eRate, e2pe );

        if (picType != MFX_FRAMETYPE_B)
        {
            m_ctx.LastNonBFrameSize = bitsEncoded;
            if (picType == MFX_FRAMETYPE_I)
                m_ctx.eRateSH = eRate;
            else
                m_ctx.eRate = eRate;

        }

        if (m_avg.get())
        {
            m_avg->UpdateSlidingWindow(bitsEncoded, m_ctx.encOrder, m_ctx.bPanic, bSHStart || picType == MFX_FRAMETYPE_I,frame_par->NumRecode);
        }

        m_ctx.totalDiviation += ((mfxF64)bitsEncoded - m_par.inputBitsPerFrame);

        //printf("-- %d (%d)) Total deviation %f, old scene %d, bNeedUpdateQP %d, m_ctx.Quant %d, type %d\n", frame_par->EncodedOrder, frame_par->DisplayOrder,m_ctx.totalDiviation, oldScene , bNeedUpdateQP, m_ctx.Quant,picType);

        if (!m_ctx.bPanic&& (!oldScene) && bNeedUpdateQP)
        {
            mfxI32 quant_new = m_ctx.Quant;
            //Update QP

            mfxF64 totDiv = m_ctx.totalDiviation;
            mfxF64 dequant_new = m_ctx.dQuantAb*pow(m_par.inputBitsPerFrame/m_ctx.fAbLong, 1.2);
            mfxF64 bAbPreriod = m_par.bAbPeriod;

            if (m_par.HRDConformance != MFX_BRC_NO_HRD && totDiv > 0 )
            {
                if (m_par.rateControlMethod == MFX_RATECONTROL_VBR)
                {
                    totDiv = std::max(totDiv, m_hrd.GetBufferDiviation(m_par.targetbps));
                }
                bAbPreriod = (mfxF64)(m_par.bPyr? 4 : 3)*(mfxF64)m_hrd.GetMaxFrameSize() / m_par.inputBitsPerFrame*GetAbPeriodCoeff(m_ctx.encOrder - m_ctx.LastIEncOrder, m_par.gopPicSize) ;
                bAbPreriod = mfx::clamp(bAbPreriod , m_par.bAbPeriod/10, m_par.bAbPeriod);
            }
            quant_new = GetNewQPTotal(totDiv / bAbPreriod / (mfxF64)m_par.inputBitsPerFrame, dequant_new, m_ctx.QuantMin, m_ctx.QuantMax, m_ctx.Quant, m_par.bPyr && m_par.bRec, bSHStart && m_ctx.bToRecode == 0);
            //printf("    ===%d quant old %d quant_new %d, bitsEncoded %d m_ctx.QuantMin %d m_ctx.QuantMax %d\n", frame_par->EncodedOrder, m_ctx.Quant, quant_new, bitsEncoded, m_ctx.QuantMin, m_ctx.QuantMax);

            if (bMaxFrameSizeMode)
            {
                mfxF64 targetMax = ((mfxF64)m_par.maxFrameSizeInBits*((bSHStart || picType == MFX_FRAMETYPE_I) ? 0.95 : 0.9));
                mfxF64 targetMin = ((mfxF64)m_par.maxFrameSizeInBits*((bSHStart || picType == MFX_FRAMETYPE_I) ? 0.9  : 0.8 /*0.75 : 0.5*/));
                mfxI32 QuantNewMin = GetNewQP(bitsEncoded, targetMax, m_ctx.QuantMin, m_ctx.QuantMax, m_ctx.Quant, m_par.quantOffset, 1,false, false);
                mfxI32 QuantNewMax = GetNewQP(bitsEncoded, targetMin, m_ctx.QuantMin, m_ctx.QuantMax, m_ctx.Quant, m_par.quantOffset, 1,false, false);
                mfxI32 quant_corrected = m_ctx.Quant;

                if (quant_corrected < QuantNewMin - 3)
                    quant_corrected += 2;
                if (quant_corrected < QuantNewMin)
                    quant_corrected ++;
                else if (quant_corrected > QuantNewMax + 3)
                    quant_corrected -= 2;
                else if (quant_corrected > QuantNewMax)
                    quant_corrected--;

                //printf("   QuantNewMin %d, QuantNewMax %d, m_ctx.Quant %d, new %d (%d)\n", QuantNewMin, QuantNewMax, m_ctx.Quant, quant_corrected, quant_new);

                quant_new = mfx::clamp(quant_corrected, m_ctx.QuantMin, m_ctx.QuantMax);
            }
            if ((quant_new - m_ctx.Quant)* (quant_new - GetCurQP (picType, layer)) > 0) // this check is actual for async scheme
            {
                //printf("   Update QP %d: totalDiviation %f, bAbPreriod %f (%f), QP %d (%d %d), qp_new %d (qpY %d), type %d, dequant_new %f (%f) , m_ctx.fAbLong %f, m_par.inputBitsPerFrame %f (%f)\n", frame_par->EncodedOrder, totDiv, bAbPreriod, GetAbPeriodCoeff(m_ctx.encOrder - m_ctx.LastIEncOrder, m_par.gopPicSize), m_ctx.Quant, m_ctx.QuantMin, m_ctx.QuantMax, quant_new, qpY, picType, 1.0/dequant_new, 1.0/m_ctx.dQuantAb, m_ctx.fAbLong, m_par.inputBitsPerFrame, m_par.inputBitsPerFrame/m_ctx.fAbLong);
                UpdateQPParams(quant_new ,picType, m_ctx, 0, m_ctx.QuantMin , m_ctx.QuantMax, layer);
            }
        }
        m_ctx.bToRecode = 0;
    }
    return sts;

}

mfxStatus ExtBRC::GetFrameCtrl (mfxBRCFrameParam* par, mfxBRCFrameCtrl* ctrl)
{
    MFX_CHECK_NULL_PTR2(par, ctrl);
    MFX_CHECK(m_bInit, MFX_ERR_NOT_INITIALIZED);

    mfxI32 qp = 0;
    if (par->EncodedOrder == m_ctx.encOrder)
    {
        qp = m_ctx.Quant;
    }
    else
    {
        mfxU16 type = GetFrameType(par->FrameType,par->PyramidLayer, m_par.gopRefDist);
        qp = GetCurQP (type, par->PyramidLayer);
    }
    ctrl->QpY = qp - m_par.quantOffset;
    //printf("ctrl->QpY %d, qp %d quantOffset %d\n", ctrl->QpY , qp , m_par.quantOffset);
    return MFX_ERR_NONE;
}

mfxStatus ExtBRC::Reset(mfxVideoParam *par )
{
    mfxStatus sts = MFX_ERR_NONE;
    MFX_CHECK_NULL_PTR1(par);
    MFX_CHECK(m_bInit, MFX_ERR_NOT_INITIALIZED);

    mfxExtEncoderResetOption  * pRO = (mfxExtEncoderResetOption *)Hevc_GetExtBuffer(par->ExtParam, par->NumExtParam, MFX_EXTBUFF_ENCODER_RESET_OPTION);
    if (pRO && pRO->StartNewSequence == MFX_CODINGOPTION_ON)
    {
        Close();
        sts = Init(par);
    }
    else
    {
        bool brcReset = false;
        bool slidingWindowReset = false;

        sts = m_par.GetBRCResetType(par, false, brcReset, slidingWindowReset);
        MFX_CHECK_STS(sts);

        if (brcReset)
        {
            sts = m_par.Init(par);
            MFX_CHECK_STS(sts);

            m_ctx.Quant = (mfxI32)(1. / m_ctx.dQuantAb * pow(m_ctx.fAbLong / m_par.inputBitsPerFrame, 0.32) + 0.5);
            m_ctx.Quant = mfx::clamp(m_ctx.Quant, m_par.quantMinI, m_par.quantMaxI);

            UpdateQPParams(m_ctx.Quant, MFX_FRAMETYPE_I, m_ctx, 0, m_par.quantMinI, m_par.quantMaxI, 0);

            m_ctx.dQuantAb = 1. / m_ctx.Quant;
            m_ctx.fAbLong = m_par.inputBitsPerFrame;
            m_ctx.fAbShort = m_par.inputBitsPerFrame;

            if (slidingWindowReset)
            {
                m_avg.reset(new AVGBitrate(m_par.WinBRCSize, (mfxU32)(m_par.WinBRCMaxAvgKbps*1000.0 / m_par.frameRate), (mfxU32)m_par.inputBitsPerFrame));
                MFX_CHECK_NULL_PTR1(m_avg.get());
            }
        }
    }
    return sts;
}

#endif
