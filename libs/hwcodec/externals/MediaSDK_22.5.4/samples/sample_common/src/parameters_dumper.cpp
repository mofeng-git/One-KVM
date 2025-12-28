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

#include "parameters_dumper.h"
#include <stdio.h>
#include <string>
#include <fstream>
#include <vector>

#include "vm/strings_defs.h"

#include "mfxstructures.h"
#include "mfxvideo.h"
#include "mfxvideo++.h"
#include "mfxjpeg.h"
#include "mfxplugin.h"

#include "sample_types.h"
#include "mfxvp8.h"
#include "mfxmvc.h"
#include "mfxla.h"

#ifndef MFX_VERSION
#error MFX_VERSION not defined
#endif

#define START_PROC_ARRAY(arrName) for(unsigned int arrIdx=0;arrIdx<(sizeof(info.arrName)/sizeof(info.arrName[0]));arrIdx++){
#define START_PROC_ARRAY_SIZE(arrName,numElems) for(unsigned int arrIdx=0;arrIdx<info.numElems;arrIdx++){
#define SERIALIZE_INFO_ELEMENT(arrName,name) {sstr<<prefix<<MSDK_STRING(#arrName)<<MSDK_STRING("[")<<arrIdx<<MSDK_STRING("].")<<MSDK_STRING(#name)<<MSDK_STRING(":")<<info.arrName[arrIdx].name<<std::endl;}
#define SERIALIZE_INFO_ARRAY_ELEMENT(arrName,name) {sstr<<prefix<<MSDK_STRING(#arrName)<<MSDK_STRING("[")<<arrIdx<<MSDK_STRING("].")<<MSDK_STRING(#name)<<MSDK_STRING(":");for(unsigned int idx=0;idx<(sizeof(info.arrName[0].name)/sizeof(info.arrName[0].name[0]));idx++){sstr<<info.arrName[arrIdx].name[idx]<<" ";};sstr<<std::endl;}
#define END_PROC_ARRAY }
#define SERIALIZE_INFO(name) {sstr<<prefix<<MSDK_STRING(#name)<<MSDK_STRING(":")<<info.name<<std::endl;}
#define SERIALIZE_INFO_ARRAY(name) {sstr<<prefix<<MSDK_STRING(#name)<<MSDK_STRING(":");for(unsigned int idx=0;idx<(sizeof(info.name)/sizeof(info.name[0]));idx++){sstr<<info.name[idx]<<" ";};sstr<<std::endl;}
#define SERIALIZE_INFO_MEMORY(name,numElems) {sstr<<prefix<<MSDK_STRING(#name)<<MSDK_STRING(":");for(unsigned int idx=0;idx<info.numElems;idx++){sstr<<info.name[idx]<<" ";};sstr<<std::endl;}

void CParametersDumper::SerializeFrameInfoStruct(msdk_ostream& sstr,msdk_string prefix,mfxFrameInfo& info)
{
    SERIALIZE_INFO_ARRAY(reserved);
    SERIALIZE_INFO(reserved4);
    SERIALIZE_INFO(BitDepthLuma);
    SERIALIZE_INFO(BitDepthChroma);
    SERIALIZE_INFO(Shift);

    SERIALIZE_INFO(FrameId.TemporalId);
    SERIALIZE_INFO(FrameId.PriorityId);
    SERIALIZE_INFO(FrameId.DependencyId);
    SERIALIZE_INFO(FrameId.QualityId);
    SERIALIZE_INFO(FrameId.ViewId);

    SERIALIZE_INFO(FourCC);
    char strFourCC[5]={};
    MSDK_MEMCPY(strFourCC,&info.FourCC,4);
    sstr<<prefix<<"FourCC(string)"<<":"<<strFourCC<<std::endl;

    SERIALIZE_INFO(Width);
    SERIALIZE_INFO(Height);

    SERIALIZE_INFO(CropX);
    SERIALIZE_INFO(CropY);
    SERIALIZE_INFO(CropW);
    SERIALIZE_INFO(CropH);
    SERIALIZE_INFO(BufferSize);
    SERIALIZE_INFO(reserved5);

    SERIALIZE_INFO(FrameRateExtN);
    SERIALIZE_INFO(FrameRateExtD);
    SERIALIZE_INFO(reserved3);

    SERIALIZE_INFO(AspectRatioW);
    SERIALIZE_INFO(AspectRatioH);

    SERIALIZE_INFO(PicStruct);
    SERIALIZE_INFO(ChromaFormat);
    SERIALIZE_INFO(reserved2);
}

void CParametersDumper::SerializeMfxInfoMFXStruct(msdk_ostream& sstr,msdk_string prefix,mfxInfoMFX& info)
{
    SERIALIZE_INFO_ARRAY(reserved);

    SERIALIZE_INFO(LowPower);
    SERIALIZE_INFO(BRCParamMultiplier);

    SerializeFrameInfoStruct(sstr,prefix+MSDK_STRING("FrameInfo."),info.FrameInfo);
    SERIALIZE_INFO(CodecId);
    char strID[5]={};
    MSDK_MEMCPY(strID,&info.CodecId,4);
    sstr<<prefix<<"CodecID(string)"<<":"<<strID<<std::endl;

    SERIALIZE_INFO(CodecProfile);
    SERIALIZE_INFO(CodecLevel);
    SERIALIZE_INFO(NumThread);

    SERIALIZE_INFO(TargetUsage);

    SERIALIZE_INFO(GopPicSize);
    SERIALIZE_INFO(GopRefDist);
    SERIALIZE_INFO(GopOptFlag);
    SERIALIZE_INFO(IdrInterval);

    SERIALIZE_INFO(RateControlMethod);
    SERIALIZE_INFO(InitialDelayInKB);
    SERIALIZE_INFO(QPI);
    SERIALIZE_INFO(Accuracy);
    SERIALIZE_INFO(BufferSizeInKB);
    SERIALIZE_INFO(TargetKbps);
    SERIALIZE_INFO(QPP);
    SERIALIZE_INFO(ICQQuality);
    SERIALIZE_INFO(MaxKbps);
    SERIALIZE_INFO(QPB);
    SERIALIZE_INFO(Convergence);

    SERIALIZE_INFO(NumSlice);
    SERIALIZE_INFO(NumRefFrame);
    SERIALIZE_INFO(EncodedOrder);

    /* Decoding Options */
    SERIALIZE_INFO(DecodedOrder);
    SERIALIZE_INFO(ExtendedPicStruct);
    SERIALIZE_INFO(TimeStampCalc);
    SERIALIZE_INFO(SliceGroupsPresent);
    SERIALIZE_INFO(MaxDecFrameBuffering);
    SERIALIZE_INFO(EnableReallocRequest);

    /* JPEG Decoding Options */
    SERIALIZE_INFO(JPEGChromaFormat);
    SERIALIZE_INFO(Rotation);
    SERIALIZE_INFO(JPEGColorFormat);
    SERIALIZE_INFO(InterleavedDec);
    SERIALIZE_INFO_ARRAY(SamplingFactorH);
    SERIALIZE_INFO_ARRAY(SamplingFactorV);

    /* JPEG Encoding Options */
    SERIALIZE_INFO(Interleaved);
    SERIALIZE_INFO(Quality);
    SERIALIZE_INFO(RestartInterval);
}

void CParametersDumper::SerializeExtensionBuffer(msdk_ostream& sstr,msdk_string prefix,mfxExtBuffer* pExtBuffer)
{
    char name[6]="    .";
    MSDK_MEMCPY(name,&pExtBuffer->BufferId,4);
    std::string strName(name);
    prefix+=msdk_string(strName.begin(),strName.end());

    // Serializing header
    {
        mfxExtBuffer& info = *pExtBuffer;
        SERIALIZE_INFO(BufferId);
        SERIALIZE_INFO(BufferSz);
    }

    // Serializing particular Ext buffer.
    switch(pExtBuffer->BufferId)
    {
    case MFX_EXTBUFF_THREADS_PARAM:
        {
            mfxExtThreadsParam& info = *(mfxExtThreadsParam*)pExtBuffer;
            SERIALIZE_INFO(NumThread);
            SERIALIZE_INFO(SchedulingType);
            SERIALIZE_INFO(Priority);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_JPEG_QT:
        {
            mfxExtJPEGQuantTables& info = *(mfxExtJPEGQuantTables*)pExtBuffer;
            SERIALIZE_INFO_ARRAY(reserved);
            SERIALIZE_INFO(NumTable);
            SERIALIZE_INFO_ARRAY(Qm[0]);
            SERIALIZE_INFO_ARRAY(Qm[1]);
            SERIALIZE_INFO_ARRAY(Qm[2]);
            SERIALIZE_INFO_ARRAY(Qm[3]);
        }
        break;
    case MFX_EXTBUFF_JPEG_HUFFMAN:
        {
            mfxExtJPEGHuffmanTables& info = *(mfxExtJPEGHuffmanTables*)pExtBuffer;
            SERIALIZE_INFO_ARRAY(reserved);
            SERIALIZE_INFO(NumDCTable);
            SERIALIZE_INFO(NumACTable);
            START_PROC_ARRAY(DCTables)
                SERIALIZE_INFO_ARRAY_ELEMENT(DCTables,Bits);
                SERIALIZE_INFO_ARRAY_ELEMENT(DCTables,Values);
            END_PROC_ARRAY
            START_PROC_ARRAY(DCTables)
                SERIALIZE_INFO_ARRAY_ELEMENT(ACTables,Bits);
                SERIALIZE_INFO_ARRAY_ELEMENT(ACTables,Values);
            END_PROC_ARRAY
        }
        break;
    case MFX_EXTBUFF_LOOKAHEAD_CTRL:
        {
            mfxExtLAControl& info = *(mfxExtLAControl*)pExtBuffer;
            SERIALIZE_INFO(LookAheadDepth);
            SERIALIZE_INFO(DependencyDepth);
            SERIALIZE_INFO(DownScaleFactor);
            SERIALIZE_INFO(BPyramid);
            SERIALIZE_INFO_ARRAY(reserved1);
            SERIALIZE_INFO(NumOutStream);
            START_PROC_ARRAY(OutStream)
                SERIALIZE_INFO_ELEMENT(OutStream,Width);
                SERIALIZE_INFO_ELEMENT(OutStream,Height);
                SERIALIZE_INFO_ARRAY_ELEMENT(OutStream,reserved2);
            END_PROC_ARRAY
        }
        break;
    case MFX_EXTBUFF_LOOKAHEAD_STAT:
        {
            mfxExtLAFrameStatistics& info = *(mfxExtLAFrameStatistics*)pExtBuffer;
            SERIALIZE_INFO_ARRAY(reserved);
            SERIALIZE_INFO(NumStream);
            SERIALIZE_INFO(NumFrame);
            //DO_MANUALLY:     mfxLAFrameInfo   *FrameStat; //frame statistics
            //DO_MANUALLY:     mfxFrameSurface1 *OutSurface; //reordered surface
        }
        break;
    case MFX_EXTBUFF_MVC_SEQ_DESC:
        {
            mfxExtMVCSeqDesc& info = *(mfxExtMVCSeqDesc*)pExtBuffer;
            SERIALIZE_INFO(NumView);
            SERIALIZE_INFO(NumViewAlloc);
            //DO_MANUALLY:     mfxMVCViewDependency *View;
            SERIALIZE_INFO(NumViewId);
            SERIALIZE_INFO(NumViewIdAlloc);
            SERIALIZE_INFO_MEMORY(ViewId,NumViewId);
            SERIALIZE_INFO(NumOP);
            SERIALIZE_INFO(NumOPAlloc);
            //DO_MANUALLY:     mfxMVCOperationPoint *OP;
            SERIALIZE_INFO(NumRefsTotal);
            SERIALIZE_INFO_ARRAY(Reserved);
        }
        break;
    case MFX_EXTBUFF_MVC_TARGET_VIEWS:
        {
            mfxExtMVCTargetViews & info = *(mfxExtMVCTargetViews *)pExtBuffer;
            SERIALIZE_INFO(TemporalId);
            SERIALIZE_INFO(NumView);
            SERIALIZE_INFO_ARRAY(ViewId);
        }
        break;
    case MFX_EXTBUFF_VPP_PICSTRUCT_DETECTION:
        {
            // No structure accociated with MFX_EXTBUFF_VPP_PICSTRUCT_DETECTION
        }
        break;
    case MFX_EXTBUFF_CODING_OPTION:
        {
            mfxExtCodingOption& info = *(mfxExtCodingOption*)pExtBuffer;
            SERIALIZE_INFO(reserved1);
            SERIALIZE_INFO(RateDistortionOpt);
            SERIALIZE_INFO(MECostType);
            SERIALIZE_INFO(MESearchType);
            SERIALIZE_INFO(MVSearchWindow.x);
            SERIALIZE_INFO(MVSearchWindow.y);
            SERIALIZE_INFO(EndOfSequence);
            SERIALIZE_INFO(FramePicture);
            SERIALIZE_INFO(CAVLC);
            SERIALIZE_INFO_ARRAY(reserved2);
            SERIALIZE_INFO(RecoveryPointSEI);
            SERIALIZE_INFO(ViewOutput);
            SERIALIZE_INFO(NalHrdConformance);
            SERIALIZE_INFO(SingleSeiNalUnit);
            SERIALIZE_INFO(VuiVclHrdParameters);
            SERIALIZE_INFO(RefPicListReordering);
            SERIALIZE_INFO(ResetRefList);
            SERIALIZE_INFO(RefPicMarkRep);
            SERIALIZE_INFO(FieldOutput);
            SERIALIZE_INFO(IntraPredBlockSize);
            SERIALIZE_INFO(InterPredBlockSize);
            SERIALIZE_INFO(MVPrecision);
            SERIALIZE_INFO(MaxDecFrameBuffering);
            SERIALIZE_INFO(AUDelimiter);
            SERIALIZE_INFO(EndOfStream);
            SERIALIZE_INFO(PicTimingSEI);
            SERIALIZE_INFO(VuiNalHrdParameters);
        }
        break;
    case MFX_EXTBUFF_CODING_OPTION2:
        {
            mfxExtCodingOption2& info = *(mfxExtCodingOption2*)pExtBuffer;
            SERIALIZE_INFO(IntRefType);
            SERIALIZE_INFO(IntRefCycleSize);
            SERIALIZE_INFO(IntRefQPDelta);
            SERIALIZE_INFO(MaxFrameSize);
            SERIALIZE_INFO(MaxSliceSize);
            SERIALIZE_INFO(BitrateLimit);
            SERIALIZE_INFO(MBBRC);
            SERIALIZE_INFO(ExtBRC);
            SERIALIZE_INFO(LookAheadDepth);
            SERIALIZE_INFO(Trellis);
            SERIALIZE_INFO(RepeatPPS);
            SERIALIZE_INFO(BRefType);
            SERIALIZE_INFO(AdaptiveI);
            SERIALIZE_INFO(AdaptiveB);
            SERIALIZE_INFO(LookAheadDS);
            SERIALIZE_INFO(NumMbPerSlice);
            SERIALIZE_INFO(SkipFrame);
            SERIALIZE_INFO(MinQPI);
            SERIALIZE_INFO(MaxQPI);
            SERIALIZE_INFO(MinQPP);
            SERIALIZE_INFO(MaxQPP);
            SERIALIZE_INFO(MinQPB);
            SERIALIZE_INFO(MaxQPB);
            SERIALIZE_INFO(FixedFrameRate);
            SERIALIZE_INFO(DisableDeblockingIdc);
            SERIALIZE_INFO(DisableVUI);
            SERIALIZE_INFO(BufferingPeriodSEI);
            SERIALIZE_INFO(EnableMAD);
            SERIALIZE_INFO(UseRawRef);
        }
        break;
    case MFX_EXTBUFF_CODING_OPTION3:
        {
            mfxExtCodingOption3& info = *(mfxExtCodingOption3*)pExtBuffer;
            SERIALIZE_INFO(NumSliceI);
            SERIALIZE_INFO(NumSliceP);
            SERIALIZE_INFO(NumSliceB);
            SERIALIZE_INFO(WinBRCMaxAvgKbps);
            SERIALIZE_INFO(WinBRCSize);
            SERIALIZE_INFO(QVBRQuality);
            SERIALIZE_INFO(EnableMBQP);
            SERIALIZE_INFO(IntRefCycleDist);
            SERIALIZE_INFO(DirectBiasAdjustment);
            SERIALIZE_INFO(GlobalMotionBiasAdjustment);
            SERIALIZE_INFO(MVCostScalingFactor);
            SERIALIZE_INFO(MBDisableSkipMap);
            SERIALIZE_INFO(WeightedPred);
            SERIALIZE_INFO(WeightedBiPred);
            SERIALIZE_INFO(AspectRatioInfoPresent);
            SERIALIZE_INFO(OverscanInfoPresent);
            SERIALIZE_INFO(OverscanAppropriate);
            SERIALIZE_INFO(TimingInfoPresent);
            SERIALIZE_INFO(BitstreamRestriction);
            SERIALIZE_INFO(LowDelayHrd);
            SERIALIZE_INFO(MotionVectorsOverPicBoundaries);
            SERIALIZE_INFO(ScenarioInfo);
            SERIALIZE_INFO(ContentInfo);
            SERIALIZE_INFO(PRefType);
            SERIALIZE_INFO(FadeDetection);
            SERIALIZE_INFO(GPB);
            SERIALIZE_INFO(MaxFrameSizeI);
            SERIALIZE_INFO(MaxFrameSizeP);
#if (MFX_VERSION >= 1027)
            SERIALIZE_INFO(TargetBitDepthLuma);
            SERIALIZE_INFO(TargetBitDepthChroma);
#endif
#if (MFX_VERSION >= MFX_VERSION_NEXT)
            SERIALIZE_INFO(Log2MaxMvLengthHorizontal);
            SERIALIZE_INFO(Log2MaxMvLengthVertical);
#else
            SERIALIZE_INFO_ARRAY(reserved1);
#endif
            SERIALIZE_INFO(EnableQPOffset);
            SERIALIZE_INFO_ARRAY(QPOffset);
            SERIALIZE_INFO_ARRAY(NumRefActiveP);
            SERIALIZE_INFO_ARRAY(NumRefActiveBL0);
            SERIALIZE_INFO_ARRAY(NumRefActiveBL1);
            SERIALIZE_INFO(BRCPanicMode);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_VPP_DONOTUSE:
        {
            mfxExtVPPDoNotUse& info = *(mfxExtVPPDoNotUse*)pExtBuffer;
            SERIALIZE_INFO(NumAlg);
            SERIALIZE_INFO_MEMORY(AlgList,NumAlg);
        }
        break;
    case MFX_EXTBUFF_VPP_DENOISE:
        {
            mfxExtVPPDenoise& info = *(mfxExtVPPDenoise*)pExtBuffer;
            SERIALIZE_INFO(DenoiseFactor);
        }
        break;
    case MFX_EXTBUFF_VPP_DETAIL:
        {
            mfxExtVPPDetail& info = *(mfxExtVPPDetail*)pExtBuffer;
            SERIALIZE_INFO(DetailFactor);
        }
        break;
    case MFX_EXTBUFF_VPP_PROCAMP:
        {
            mfxExtVPPProcAmp& info = *(mfxExtVPPProcAmp*)pExtBuffer;
            SERIALIZE_INFO(Brightness);
            SERIALIZE_INFO(Contrast);
            SERIALIZE_INFO(Hue);
            SERIALIZE_INFO(Saturation);
        }
        break;
    case MFX_EXTBUFF_VPP_AUXDATA:
        {
            mfxExtVppAuxData& info = *(mfxExtVppAuxData*)pExtBuffer;
            SERIALIZE_INFO(SpatialComplexity);
            SERIALIZE_INFO(TemporalComplexity);
            SERIALIZE_INFO(PicStruct);
            SERIALIZE_INFO_ARRAY(reserved);
            SERIALIZE_INFO(SceneChangeRate);
            SERIALIZE_INFO(RepeatedFrame);
        }
        break;
    case MFX_EXTBUFF_CODING_OPTION_SPSPPS:
        {
            mfxExtCodingOptionSPSPPS& info = *(mfxExtCodingOptionSPSPPS*)pExtBuffer;
            SERIALIZE_INFO(SPSBuffer);
            SERIALIZE_INFO(PPSBuffer);
            SERIALIZE_INFO(SPSBufSize);
            SERIALIZE_INFO(PPSBufSize);
            SERIALIZE_INFO(SPSId);
            SERIALIZE_INFO(PPSId);
        }
        break;
    case MFX_EXTBUFF_CODING_OPTION_VPS:
        {
            mfxExtCodingOptionVPS& info = *(mfxExtCodingOptionVPS*)pExtBuffer;
            SERIALIZE_INFO(VPSBuffer);
            SERIALIZE_INFO(reserved1);
            SERIALIZE_INFO(VPSBufSize);
            SERIALIZE_INFO(VPSId);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_VPP_VIDEO_SIGNAL_INFO:
        {
            mfxExtVideoSignalInfo& info = *(mfxExtVideoSignalInfo*)pExtBuffer;
            SERIALIZE_INFO(VideoFormat);
            SERIALIZE_INFO(VideoFullRange);
            SERIALIZE_INFO(ColourDescriptionPresent);
            SERIALIZE_INFO(ColourPrimaries);
            SERIALIZE_INFO(TransferCharacteristics);
            SERIALIZE_INFO(MatrixCoefficients);
        }
        break;
    case MFX_EXTBUFF_VPP_DOUSE:
        {
            mfxExtVPPDoUse& info = *(mfxExtVPPDoUse*)pExtBuffer;
            SERIALIZE_INFO(NumAlg);
            SERIALIZE_INFO(AlgList);
        }
        break;
    case MFX_EXTBUFF_OPAQUE_SURFACE_ALLOCATION:
        {
            mfxExtOpaqueSurfaceAlloc& info = *(mfxExtOpaqueSurfaceAlloc*)pExtBuffer;
            SERIALIZE_INFO_ARRAY(reserved1);
            SERIALIZE_INFO_ARRAY(In.reserved2);
            SERIALIZE_INFO(In.Type);
            SERIALIZE_INFO(In.NumSurface);
            SERIALIZE_INFO_ARRAY(Out.reserved2);
            SERIALIZE_INFO(Out.Type);
            SERIALIZE_INFO(Out.NumSurface);
        }
        break;
    case MFX_EXTBUFF_AVC_REFLIST_CTRL:
        {
            mfxExtAVCRefListCtrl& info = *(mfxExtAVCRefListCtrl*)pExtBuffer;
            SERIALIZE_INFO(NumRefIdxL0Active);
            SERIALIZE_INFO(NumRefIdxL1Active);
            START_PROC_ARRAY(PreferredRefList)
                SERIALIZE_INFO_ELEMENT(PreferredRefList,FrameOrder);
                SERIALIZE_INFO_ELEMENT(PreferredRefList,PicStruct);
                SERIALIZE_INFO_ELEMENT(PreferredRefList,ViewId);
                SERIALIZE_INFO_ELEMENT(PreferredRefList,LongTermIdx);
                SERIALIZE_INFO_ARRAY_ELEMENT(PreferredRefList,reserved);
            END_PROC_ARRAY

            START_PROC_ARRAY(RejectedRefList)
                SERIALIZE_INFO_ELEMENT(RejectedRefList,FrameOrder);
                SERIALIZE_INFO_ELEMENT(RejectedRefList,PicStruct);
                SERIALIZE_INFO_ELEMENT(RejectedRefList,ViewId);
                SERIALIZE_INFO_ELEMENT(RejectedRefList,LongTermIdx);
                SERIALIZE_INFO_ARRAY_ELEMENT(RejectedRefList,reserved);
            END_PROC_ARRAY

            START_PROC_ARRAY(LongTermRefList)
                SERIALIZE_INFO_ELEMENT(LongTermRefList,FrameOrder);
                SERIALIZE_INFO_ELEMENT(LongTermRefList,PicStruct);
                SERIALIZE_INFO_ELEMENT(LongTermRefList,ViewId);
                SERIALIZE_INFO_ELEMENT(LongTermRefList,LongTermIdx);
                SERIALIZE_INFO_ARRAY_ELEMENT(LongTermRefList,reserved);
            END_PROC_ARRAY

            SERIALIZE_INFO(ApplyLongTermIdx);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_VPP_FRAME_RATE_CONVERSION:
        {
            mfxExtVPPFrameRateConversion& info = *(mfxExtVPPFrameRateConversion*)pExtBuffer;
            SERIALIZE_INFO(Algorithm);
            SERIALIZE_INFO(reserved);
            SERIALIZE_INFO_ARRAY(reserved2);
        }
        break;
    case MFX_EXTBUFF_VPP_IMAGE_STABILIZATION:
        {
            mfxExtVPPImageStab& info = *(mfxExtVPPImageStab*)pExtBuffer;
            SERIALIZE_INFO(Mode);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_PICTURE_TIMING_SEI:
        {
            mfxExtPictureTimingSEI& info = *(mfxExtPictureTimingSEI*)pExtBuffer;
            SERIALIZE_INFO_ARRAY(reserved);

            SERIALIZE_INFO(TimeStamp[0].ClockTimestampFlag);
            SERIALIZE_INFO(TimeStamp[0].CtType);
            SERIALIZE_INFO(TimeStamp[0].NuitFieldBasedFlag);
            SERIALIZE_INFO(TimeStamp[0].CountingType);
            SERIALIZE_INFO(TimeStamp[0].FullTimestampFlag);
            SERIALIZE_INFO(TimeStamp[0].DiscontinuityFlag);
            SERIALIZE_INFO(TimeStamp[0].CntDroppedFlag);
            SERIALIZE_INFO(TimeStamp[0].NFrames);
            SERIALIZE_INFO(TimeStamp[0].SecondsFlag);
            SERIALIZE_INFO(TimeStamp[0].MinutesFlag);
            SERIALIZE_INFO(TimeStamp[0].HoursFlag);
            SERIALIZE_INFO(TimeStamp[0].SecondsValue);
            SERIALIZE_INFO(TimeStamp[0].MinutesValue);
            SERIALIZE_INFO(TimeStamp[0].HoursValue);
            SERIALIZE_INFO(TimeStamp[0].TimeOffset);

            SERIALIZE_INFO(TimeStamp[1].ClockTimestampFlag);
            SERIALIZE_INFO(TimeStamp[1].CtType);
            SERIALIZE_INFO(TimeStamp[1].NuitFieldBasedFlag);
            SERIALIZE_INFO(TimeStamp[1].CountingType);
            SERIALIZE_INFO(TimeStamp[1].FullTimestampFlag);
            SERIALIZE_INFO(TimeStamp[1].DiscontinuityFlag);
            SERIALIZE_INFO(TimeStamp[1].CntDroppedFlag);
            SERIALIZE_INFO(TimeStamp[1].NFrames);
            SERIALIZE_INFO(TimeStamp[1].SecondsFlag);
            SERIALIZE_INFO(TimeStamp[1].MinutesFlag);
            SERIALIZE_INFO(TimeStamp[1].HoursFlag);
            SERIALIZE_INFO(TimeStamp[1].SecondsValue);
            SERIALIZE_INFO(TimeStamp[1].MinutesValue);
            SERIALIZE_INFO(TimeStamp[1].HoursValue);
            SERIALIZE_INFO(TimeStamp[1].TimeOffset);

            SERIALIZE_INFO(TimeStamp[2].ClockTimestampFlag);
            SERIALIZE_INFO(TimeStamp[2].CtType);
            SERIALIZE_INFO(TimeStamp[2].NuitFieldBasedFlag);
            SERIALIZE_INFO(TimeStamp[2].CountingType);
            SERIALIZE_INFO(TimeStamp[2].FullTimestampFlag);
            SERIALIZE_INFO(TimeStamp[2].DiscontinuityFlag);
            SERIALIZE_INFO(TimeStamp[2].CntDroppedFlag);
            SERIALIZE_INFO(TimeStamp[2].NFrames);
            SERIALIZE_INFO(TimeStamp[2].SecondsFlag);
            SERIALIZE_INFO(TimeStamp[2].MinutesFlag);
            SERIALIZE_INFO(TimeStamp[2].HoursFlag);
            SERIALIZE_INFO(TimeStamp[2].SecondsValue);
            SERIALIZE_INFO(TimeStamp[2].MinutesValue);
            SERIALIZE_INFO(TimeStamp[2].HoursValue);
            SERIALIZE_INFO(TimeStamp[2].TimeOffset);
        }
        break;
    case MFX_EXTBUFF_AVC_TEMPORAL_LAYERS:
        {
            mfxExtAvcTemporalLayers& info = *(mfxExtAvcTemporalLayers*)pExtBuffer;
            SERIALIZE_INFO_ARRAY(reserved1);
            SERIALIZE_INFO(reserved2);
            SERIALIZE_INFO(BaseLayerPID);
            SERIALIZE_INFO(Layer[0].Scale);
            SERIALIZE_INFO_ARRAY(Layer[0].reserved);
            SERIALIZE_INFO(Layer[1].Scale);
            SERIALIZE_INFO_ARRAY(Layer[1].reserved);
            SERIALIZE_INFO(Layer[2].Scale);
            SERIALIZE_INFO_ARRAY(Layer[2].reserved);
            SERIALIZE_INFO(Layer[3].Scale);
            SERIALIZE_INFO_ARRAY(Layer[3].reserved);
            SERIALIZE_INFO(Layer[4].Scale);
            SERIALIZE_INFO_ARRAY(Layer[4].reserved);
            SERIALIZE_INFO(Layer[5].Scale);
            SERIALIZE_INFO_ARRAY(Layer[5].reserved);
            SERIALIZE_INFO(Layer[6].Scale);
            SERIALIZE_INFO_ARRAY(Layer[6].reserved);
            SERIALIZE_INFO(Layer[7].Scale);
            SERIALIZE_INFO_ARRAY(Layer[7].reserved);
        }
        break;
    case MFX_EXTBUFF_ENCODER_CAPABILITY:
        {
            mfxExtEncoderCapability& info = *(mfxExtEncoderCapability*)pExtBuffer;
            SERIALIZE_INFO(MBPerSec);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_ENCODER_RESET_OPTION:
        {
            mfxExtEncoderResetOption& info = *(mfxExtEncoderResetOption*)pExtBuffer;
            SERIALIZE_INFO(StartNewSequence);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_ENCODED_FRAME_INFO:
        {
            mfxExtAVCEncodedFrameInfo& info = *(mfxExtAVCEncodedFrameInfo*)pExtBuffer;
            SERIALIZE_INFO(FrameOrder);
            SERIALIZE_INFO(PicStruct);
            SERIALIZE_INFO(LongTermIdx);
            SERIALIZE_INFO(MAD);
            SERIALIZE_INFO(BRCPanicMode);
            SERIALIZE_INFO(QP);
            SERIALIZE_INFO(SecondFieldOffset);
            SERIALIZE_INFO_ARRAY(reserved);
            SERIALIZE_INFO(FrameOrder);
            SERIALIZE_INFO(PicStruct);
            SERIALIZE_INFO(LongTermIdx);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_VPP_COMPOSITE:
        {
            mfxExtVPPComposite& info = *(mfxExtVPPComposite*)pExtBuffer;
            SERIALIZE_INFO(Y);
            SERIALIZE_INFO(R);
            SERIALIZE_INFO(U);
            SERIALIZE_INFO(G);
            SERIALIZE_INFO(V);
            SERIALIZE_INFO(B);
            SERIALIZE_INFO_ARRAY(reserved1);
            SERIALIZE_INFO(NumInputStream);
            for(int i=0;i<info.NumInputStream;i++)
            {
                msdk_char streamName[32];
                msdk_sprintf(streamName,MSDK_STRING("InputStream[%d]."),i);
                SerializeVPPCompInputStream(sstr,prefix+streamName,info.InputStream[i]);
            }
        }
        break;
    case MFX_EXTBUFF_VIDEO_SIGNAL_INFO:
        {
            mfxExtVPPVideoSignalInfo& info = *(mfxExtVPPVideoSignalInfo*)pExtBuffer;
            SERIALIZE_INFO_ARRAY(reserved1);

            SERIALIZE_INFO(In.TransferMatrix);
            SERIALIZE_INFO(In.NominalRange);
            SERIALIZE_INFO_ARRAY(In.reserved2);
            SERIALIZE_INFO(Out.TransferMatrix);
            SERIALIZE_INFO(Out.NominalRange);
            SERIALIZE_INFO_ARRAY(Out.reserved2);

            SERIALIZE_INFO(TransferMatrix);
            SERIALIZE_INFO(NominalRange);
            SERIALIZE_INFO_ARRAY(reserved3);
        }
        break;
    case MFX_EXTBUFF_ENCODER_ROI:
        {
            mfxExtEncoderROI& info = *(mfxExtEncoderROI*)pExtBuffer;
            SERIALIZE_INFO(NumROI);
#if MFX_VERSION >= 1022
            SERIALIZE_INFO(ROIMode);
#endif //MFX_VERSION >= 1022
            SERIALIZE_INFO_ARRAY(reserved1);
            START_PROC_ARRAY_SIZE(ROI,NumROI)
                SERIALIZE_INFO_ELEMENT(ROI,Left);
                SERIALIZE_INFO_ELEMENT(ROI,Top);
                SERIALIZE_INFO_ELEMENT(ROI,Right);
                SERIALIZE_INFO_ELEMENT(ROI,Bottom);
                SERIALIZE_INFO_ELEMENT(ROI,Priority);
#if MFX_VERSION >= 1022
                SERIALIZE_INFO_ELEMENT(ROI,DeltaQP);
#endif //MFX_VERSION >= 1022

                SERIALIZE_INFO_ARRAY_ELEMENT(ROI,reserved2);
            END_PROC_ARRAY
        }
        break;
    case MFX_EXTBUFF_VPP_DEINTERLACING:
        {
            mfxExtVPPDeinterlacing& info = *(mfxExtVPPDeinterlacing*)pExtBuffer;
            SERIALIZE_INFO(Mode);
            SERIALIZE_INFO(TelecinePattern);
            SERIALIZE_INFO(TelecineLocation);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_AVC_REFLISTS:
        {
            mfxExtAVCRefLists& info = *(mfxExtAVCRefLists*)pExtBuffer;
            SERIALIZE_INFO(NumRefIdxL0Active);
            SERIALIZE_INFO(NumRefIdxL1Active);
            SERIALIZE_INFO_ARRAY(reserved);

            START_PROC_ARRAY(RefPicList0)
                SERIALIZE_INFO_ELEMENT(RefPicList0,FrameOrder);
                SERIALIZE_INFO_ELEMENT(RefPicList0,PicStruct);
                SERIALIZE_INFO_ELEMENT(RefPicList0,reserved);
            END_PROC_ARRAY

            START_PROC_ARRAY(RefPicList1)
                SERIALIZE_INFO_ELEMENT(RefPicList1,FrameOrder);
                SERIALIZE_INFO_ELEMENT(RefPicList1,PicStruct);
                SERIALIZE_INFO_ELEMENT(RefPicList1,reserved);
            END_PROC_ARRAY
        }
        break;
    case MFX_EXTBUFF_VPP_FIELD_PROCESSING:
        {
            mfxExtVPPFieldProcessing& info = *(mfxExtVPPFieldProcessing*)pExtBuffer;
            SERIALIZE_INFO(Mode);
            SERIALIZE_INFO(InField);
            SERIALIZE_INFO(OutField);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
#if MFX_VERSION >= 1022
    case MFX_EXTBUFF_DEC_VIDEO_PROCESSING:
        {
            mfxExtDecVideoProcessing& info = *(mfxExtDecVideoProcessing*)pExtBuffer;
            SERIALIZE_INFO(In.CropX);
            SERIALIZE_INFO(In.CropY);
            SERIALIZE_INFO(In.CropW);
            SERIALIZE_INFO(In.CropH);
            SERIALIZE_INFO_ARRAY(In.reserved);
            SERIALIZE_INFO(Out.FourCC);
            SERIALIZE_INFO(Out.ChromaFormat);
            SERIALIZE_INFO(Out.Width);
            SERIALIZE_INFO(Out.Height);
            SERIALIZE_INFO(Out.CropX);
            SERIALIZE_INFO(Out.CropY);
            SERIALIZE_INFO(Out.CropW);
            SERIALIZE_INFO(Out.CropH);
            SERIALIZE_INFO_ARRAY(Out.reserved);
        }
        break;
#endif //MFX_VERSION >= 1022
    case MFX_EXTBUFF_CHROMA_LOC_INFO:
        {
            mfxExtChromaLocInfo& info = *(mfxExtChromaLocInfo*)pExtBuffer;
            SERIALIZE_INFO(ChromaLocInfoPresentFlag);
            SERIALIZE_INFO(ChromaSampleLocTypeTopField);
            SERIALIZE_INFO(ChromaSampleLocTypeBottomField);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_MBQP:
        {
            mfxExtMBQP& info = *(mfxExtMBQP*)pExtBuffer;
            SERIALIZE_INFO_ARRAY(reserved);
            SERIALIZE_INFO(NumQPAlloc);
            SERIALIZE_INFO_MEMORY(QP,NumQPAlloc);
            SERIALIZE_INFO(reserved2);
        }
        break;
    case MFX_EXTBUFF_HEVC_TILES:
        {
            mfxExtHEVCTiles& info = *(mfxExtHEVCTiles*)pExtBuffer;
            SERIALIZE_INFO(NumTileRows);
            SERIALIZE_INFO(NumTileColumns);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_MB_DISABLE_SKIP_MAP:
        {
            mfxExtMBDisableSkipMap& info = *(mfxExtMBDisableSkipMap*)pExtBuffer;
            SERIALIZE_INFO_ARRAY(reserved);
            SERIALIZE_INFO(MapSize);
            SERIALIZE_INFO_MEMORY(Map,MapSize);
            SERIALIZE_INFO(reserved2);
        }
        break;
    case MFX_EXTBUFF_HEVC_PARAM:
        {
            mfxExtHEVCParam& info = *(mfxExtHEVCParam*)pExtBuffer;
            SERIALIZE_INFO(PicWidthInLumaSamples);
            SERIALIZE_INFO(PicHeightInLumaSamples);
            SERIALIZE_INFO(GeneralConstraintFlags);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_DECODED_FRAME_INFO:
        {
            mfxExtDecodedFrameInfo& info = *(mfxExtDecodedFrameInfo*)pExtBuffer;
            SERIALIZE_INFO(FrameType);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_TIME_CODE:
        {
            mfxExtTimeCode& info = *(mfxExtTimeCode*)pExtBuffer;
            SERIALIZE_INFO(DropFrameFlag);
            SERIALIZE_INFO(TimeCodeHours);
            SERIALIZE_INFO(TimeCodeMinutes);
            SERIALIZE_INFO(TimeCodeSeconds);
            SERIALIZE_INFO(TimeCodePictures);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_HEVC_REGION:
        {
            mfxExtHEVCRegion& info = *(mfxExtHEVCRegion*)pExtBuffer;
            SERIALIZE_INFO(RegionId);
            SERIALIZE_INFO(RegionType);
            SERIALIZE_INFO(RegionEncoding);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_PRED_WEIGHT_TABLE:
        {
            mfxExtPredWeightTable& info = *(mfxExtPredWeightTable*)pExtBuffer;
            SERIALIZE_INFO(LumaLog2WeightDenom);
            SERIALIZE_INFO(ChromaLog2WeightDenom);
            SERIALIZE_INFO_ARRAY(LumaWeightFlag[0]);
            SERIALIZE_INFO_ARRAY(LumaWeightFlag[1]);
            SERIALIZE_INFO_ARRAY(ChromaWeightFlag[0]);
            SERIALIZE_INFO_ARRAY(ChromaWeightFlag[1]);
            //DO_MANUALLY: Weights[2][32][3][2];
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_DIRTY_RECTANGLES:
        {
            mfxExtDirtyRect& info = *(mfxExtDirtyRect*)pExtBuffer;
            SERIALIZE_INFO(NumRect);
            SERIALIZE_INFO_ARRAY(reserved1);

            START_PROC_ARRAY_SIZE(Rect,NumRect)
                SERIALIZE_INFO_ELEMENT(Rect,Left);
                SERIALIZE_INFO_ELEMENT(Rect,Top);
                SERIALIZE_INFO_ELEMENT(Rect,Right);
                SERIALIZE_INFO_ELEMENT(Rect,Bottom);
                SERIALIZE_INFO_ARRAY_ELEMENT(Rect,reserved2);
            END_PROC_ARRAY
        }
        break;
    case MFX_EXTBUFF_MOVING_RECTANGLES:
        {
            mfxExtMoveRect& info = *(mfxExtMoveRect*)pExtBuffer;
            SERIALIZE_INFO(NumRect);
            SERIALIZE_INFO_ARRAY(reserved1);

            START_PROC_ARRAY_SIZE(Rect,NumRect)
                SERIALIZE_INFO_ELEMENT(Rect,DestLeft);
                SERIALIZE_INFO_ELEMENT(Rect,DestTop);
                SERIALIZE_INFO_ELEMENT(Rect,DestRight);
                SERIALIZE_INFO_ELEMENT(Rect,DestBottom);
                SERIALIZE_INFO_ELEMENT(Rect,SourceLeft);
                SERIALIZE_INFO_ELEMENT(Rect,SourceTop);
                SERIALIZE_INFO_ARRAY_ELEMENT(Rect,reserved2);
            END_PROC_ARRAY
        }
        break;
    case MFX_EXTBUFF_VPP_ROTATION:
        {
            mfxExtVPPRotation& info = *(mfxExtVPPRotation*)pExtBuffer;
            SERIALIZE_INFO(Angle);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_ENCODED_SLICES_INFO:
        {
            mfxExtEncodedSlicesInfo& info = *(mfxExtEncodedSlicesInfo*)pExtBuffer;
            SERIALIZE_INFO(SliceSizeOverflow);
            SERIALIZE_INFO(NumSliceNonCopliant);
            SERIALIZE_INFO(NumEncodedSlice);
            SERIALIZE_INFO(NumSliceSizeAlloc);
            SERIALIZE_INFO_MEMORY(SliceSize,NumSliceSizeAlloc);
            SERIALIZE_INFO(reserved1);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_VPP_SCALING:
        {
            mfxExtVPPScaling& info = *(mfxExtVPPScaling*)pExtBuffer;
            SERIALIZE_INFO(ScalingMode);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_VPP_MIRRORING:
        {
            mfxExtVPPMirroring& info = *(mfxExtVPPMirroring*)pExtBuffer;
            SERIALIZE_INFO(Type);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_MV_OVER_PIC_BOUNDARIES:
        {
            mfxExtMVOverPicBoundaries& info = *(mfxExtMVOverPicBoundaries*)pExtBuffer;
            SERIALIZE_INFO(StickTop);
            SERIALIZE_INFO(StickBottom);
            SERIALIZE_INFO(StickLeft);
            SERIALIZE_INFO(StickRight);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_VPP_COLORFILL:
        {
            mfxExtVPPColorFill& info = *(mfxExtVPPColorFill*)pExtBuffer;
            SERIALIZE_INFO(Enable);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    case MFX_EXTBUFF_VP8_CODING_OPTION:
        {
            mfxExtVP8CodingOption& info = *(mfxExtVP8CodingOption*)pExtBuffer;
            SERIALIZE_INFO(Version);
            SERIALIZE_INFO(EnableMultipleSegments);
            SERIALIZE_INFO(LoopFilterType);
            SERIALIZE_INFO_ARRAY(LoopFilterLevel);
            SERIALIZE_INFO(SharpnessLevel);
            SERIALIZE_INFO(NumTokenPartitions);
            SERIALIZE_INFO_ARRAY(LoopFilterRefTypeDelta);
            SERIALIZE_INFO_ARRAY(LoopFilterMbModeDelta);
            SERIALIZE_INFO_ARRAY(SegmentQPDelta);
            SERIALIZE_INFO_ARRAY(CoeffTypeQPDelta);
            SERIALIZE_INFO(WriteIVFHeaders);
            SERIALIZE_INFO(NumFramesForIVFHeader);
            SERIALIZE_INFO_ARRAY(reserved);
        }
        break;
    }
    // End of autogenerated code
}

void CParametersDumper::SerializeVPPCompInputStream(msdk_ostream& sstr,msdk_string prefix,mfxVPPCompInputStream& info)
{
    SERIALIZE_INFO(DstX);
    SERIALIZE_INFO(DstY);
    SERIALIZE_INFO(DstW);
    SERIALIZE_INFO(DstH);

    SERIALIZE_INFO(LumaKeyEnable);
    SERIALIZE_INFO(LumaKeyMin);
    SERIALIZE_INFO(LumaKeyMax);

    SERIALIZE_INFO(GlobalAlphaEnable);
    SERIALIZE_INFO(GlobalAlpha);

    SERIALIZE_INFO(PixelAlphaEnable);

    SERIALIZE_INFO_ARRAY(reserved2);
}


void CParametersDumper::SerializeVideoParamStruct(msdk_ostream& sstr,msdk_string sectionName,mfxVideoParam& info,bool shouldUseVPPSection)
{
    msdk_string prefix=MSDK_STRING("");

    sstr<<sectionName<<std::endl;
    SERIALIZE_INFO(AllocId);
    SERIALIZE_INFO_ARRAY(reserved);
    SERIALIZE_INFO(reserved3);
    SERIALIZE_INFO(AsyncDepth);

    if(shouldUseVPPSection)
    {
        SERIALIZE_INFO_ARRAY(vpp.reserved);
        SerializeFrameInfoStruct(sstr,MSDK_STRING("vpp.In."),info.vpp.In);
        SerializeFrameInfoStruct(sstr,MSDK_STRING("vpp.Out."),info.vpp.Out);
    }
    else
    {
        SerializeMfxInfoMFXStruct(sstr,MSDK_STRING("mfx."),info.mfx);
    }

    SERIALIZE_INFO(Protected);
    SERIALIZE_INFO(IOPattern);
    for(int i=0;i<info.NumExtParam;i++)
    {
        SerializeExtensionBuffer(sstr,MSDK_STRING("ext."),info.ExtParam[i]);
    }
    SERIALIZE_INFO(reserved2);
}

mfxStatus CParametersDumper::DumpLibraryConfiguration(msdk_string fileName, MFXVideoDECODE* pMfxDec, MFXVideoVPP* pMfxVPP, MFXVideoENCODE* pMfxEnc,
    const mfxVideoParam* pDecoderPresetParams, const mfxVideoParam* pVPPPresetParams, const mfxVideoParam* pEncoderPresetParams)
{
    try
    {
        msdk_fstream sstr(fileName.c_str(),std::fstream::out);
        sstr<<MSDK_STRING("Configuration settings (fields from API ") <<
            MFX_VERSION_MAJOR << MSDK_STRING(".") << MFX_VERSION_MINOR << MSDK_STRING(")\n");

        mfxVideoParam params;
        if(pMfxDec)
        {
            if (GetUnitParams(pMfxDec, pDecoderPresetParams,&params) == MFX_ERR_NONE)
            {
                SerializeVideoParamStruct(sstr, MSDK_STRING("*** Decoder ***"), params, false);
                ClearExtBuffs(&params);
            }
        }
        if(pMfxVPP)
        {
            if (GetUnitParams(pMfxVPP, pVPPPresetParams, &params) == MFX_ERR_NONE)
            {
                SerializeVideoParamStruct(sstr, MSDK_STRING("*** VPP ***"), params, true);
                ClearExtBuffs(&params);
            }
        }
        if(pMfxEnc)
        {
            if (GetUnitParams(pMfxEnc, pEncoderPresetParams, &params) == MFX_ERR_NONE)
            {
                SerializeVideoParamStruct(sstr, MSDK_STRING("*** Encoder ***"), params, false);
                ClearExtBuffs(&params);
            }
        }
        sstr.close();
    }
    catch(...)
    {
        msdk_printf(MSDK_STRING("Cannot save library settings into file.\n"));
        return MFX_ERR_NULL_PTR;
    }
    return MFX_ERR_NONE;
}

void CParametersDumper::ShowConfigurationDiff(msdk_ostream & str1, msdk_ostream & str2)
{
    msdk_stringstream ss1;
    ss1 << str1.rdbuf();
    msdk_stringstream ss2;
    ss2 << str2.rdbuf();

    msdk_string l, r;
    while (ss1 >> l && ss2 >> r)
    {
        if (l != r)
        {
            msdk_printf(MSDK_STRING("%s changed to %s \n"), l.c_str(), r.c_str());
        }
        else
        {
            continue;
        }
    }
}
