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

#pragma once

#include "mfxstructures.h"
#include "vm/time_defs.h"
#include "vm/strings_defs.h"
#include "math.h"
#include <vector>
#include <stdio.h>


class CTimeStatisticsReal
{
public:
    CTimeStatisticsReal()
    {
        ResetStatistics();
        start=0;
        m_bNeedDumping = false;
    }

    static msdk_tick GetFrequency()
    {
        if (!frequency)
        {
            frequency = msdk_time_get_frequency();
        }
        return frequency;
    }

    static mfxF64 ConvertToSeconds(msdk_tick elapsed)
    {
        return MSDK_GET_TIME(elapsed, 0, GetFrequency());
    }

    inline void StartTimeMeasurement()
    {
        start = msdk_time_get_tick();
    }

    inline void StopTimeMeasurement()
    {
        mfxF64 delta=GetDeltaTime();
        totalTime+=delta;
        totalTimeSquares+=delta*delta;
        // dump in ms:
        if(m_bNeedDumping)
            m_time_deltas.push_back(delta * 1000);

        if(delta<minTime)
        {
            minTime=delta;
        }

        if(delta>maxTime)
        {
            maxTime=delta;
        }
        numMeasurements++;
    }

    inline void StopTimeMeasurementWithCheck()
    {
        if(start)
        {
            StopTimeMeasurement();
        }
    }

    inline mfxF64 GetDeltaTime()
    {
        return MSDK_GET_TIME(msdk_time_get_tick(), start, GetFrequency());
    }

    inline mfxF64 GetDeltaTimeInMiliSeconds()
    {
        return GetDeltaTime() * 1000;
    }

    inline void TurnOnDumping(){m_bNeedDumping = true; }

    inline void TurnOffDumping(){m_bNeedDumping = false; }

    inline void PrintStatistics(const msdk_char* prefix)
    {
        msdk_printf(MSDK_STRING("%s Total:%.3lfms(%lld smpls),Avg %.3lfms,StdDev:%.3lfms,Min:%.3lfms,Max:%.3lfms\n"),
                prefix,totalTime,numMeasurements,
                GetAvgTime(false),GetTimeStdDev(false),
                GetMinTime(false),GetMaxTime(false));
    }

    inline mfxU64 GetNumMeasurements()
    {
        return numMeasurements;
    }

    inline mfxF64 GetAvgTime(bool inSeconds=true)
    {
        if (inSeconds)
        {
            return (numMeasurements ? totalTime / numMeasurements : 0);
        }
        else
        {
            return (numMeasurements ? totalTime / numMeasurements : 0) * 1000;
        }
    }

    inline mfxF64 GetTimeStdDev(bool inSeconds=true)
    {
        mfxF64 avg = GetAvgTime();
        mfxF64 ftmp = (numMeasurements ? sqrt(totalTimeSquares/numMeasurements-avg*avg) : 0.0);
        return inSeconds ? ftmp : ftmp * 1000;
    }

    inline mfxF64 GetMinTime(bool inSeconds=true)
    {
        return inSeconds ? minTime : minTime * 1000;
    }

    inline mfxF64 GetMaxTime(bool inSeconds=true)
    {
        return inSeconds ? maxTime : maxTime * 1000;
    }

    inline mfxF64 GetTotalTime(bool inSeconds=true)
    {
        return inSeconds ? totalTime : totalTime * 1000;
    }

    inline void ResetStatistics()
    {
        totalTime=0;
        totalTimeSquares=0;
        minTime=1E100;
        maxTime=-1;
        numMeasurements=0;
        m_time_deltas.clear();
        TurnOffDumping();
    }

protected:
    static msdk_tick frequency;

    msdk_tick start;
    mfxF64 totalTime;
    mfxF64 totalTimeSquares;
    mfxF64 minTime;
    mfxF64 maxTime;
    mfxU64 numMeasurements;
    std::vector<mfxF64> m_time_deltas;
    bool m_bNeedDumping;

};

class CTimeStatisticsDummy
{
public:
    static msdk_tick GetFrequency()
    {
        if (!frequency)
        {
            frequency = msdk_time_get_frequency();
        }
        return frequency;
    }

    static mfxF64 ConvertToSeconds(msdk_tick /*elapsed*/)
    {
        return 0;
    }

    inline void StartTimeMeasurement()
    {
    }

    inline void StopTimeMeasurement()
    {
    }

    inline void StopTimeMeasurementWithCheck()
    {
    }

    inline mfxF64 GetDeltaTime()
    {
        return 0;
    }

    inline mfxF64 GetDeltaTimeInMiliSeconds()
    {
        return 0;
    }

    inline void TurnOnDumping() {}

    inline void TurnOffDumping() {}

    inline void PrintStatistics(const msdk_char* /*prefix*/)
    {
    }

    inline mfxU64 GetNumMeasurements()
    {
        return 0;
    }

    inline mfxF64 GetAvgTime(bool)
    {
        return 0;
    }

    inline mfxF64 GetTimeStdDev(bool)
    {
        return 0;
    }

    inline mfxF64 GetMinTime(bool)
    {
        return 0;
    }

    inline mfxF64 GetMaxTime(bool)
    {
        return 0;
    }

    inline mfxF64 GetTotalTime(bool)
    {
        return  0;
    }

    inline void ResetStatistics()
    {
    }

protected:
    static msdk_tick frequency;
};

#ifdef TIME_STATS
    typedef CTimeStatisticsReal CTimeStatistics;
#else
    typedef CTimeStatisticsDummy CTimeStatistics;
#endif
