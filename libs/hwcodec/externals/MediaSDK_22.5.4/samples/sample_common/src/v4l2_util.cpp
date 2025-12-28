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

#if defined (ENABLE_V4L2_SUPPORT)

#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include <sys/ioctl.h>
#include <fcntl.h>
#include <pthread.h>
#include <poll.h>
#include <signal.h>
#include <assert.h>
#include <linux/videodev2.h>
#include "v4l2_util.h"

/* Global Declaration */
Buffer *buffers, *CurBuffers;
bool CtrlFlag = false;
int m_q[5], m_first = 0, m_last = 0, m_numInQ = 0;
pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
pthread_mutex_t empty = PTHREAD_MUTEX_INITIALIZER;

v4l2Device::v4l2Device( const char *devname,
            uint32_t width,
            uint32_t height,
            uint32_t num_buffers,
            enum AtomISPMode MipiMode,
            enum V4L2PixelFormat v4l2Format):
            m_devname(devname),
            m_height(height),
            m_width(width),
            m_num_buffers(num_buffers),
            m_MipiPort(0),
            m_MipiMode(MipiMode),
            m_v4l2Format(v4l2Format),
            m_fd(-1)
{
}

v4l2Device::~v4l2Device()
{
    if (m_fd > -1)
    {
        BYE_ON(close(m_fd) < 0, "V4L2 device close failed: %s\n", ERRSTR);
    }
}

int v4l2Device::blockIOCTL(int handle, int request, void *args)
{
    int ioctlStatus;
    do
    {
        ioctlStatus = ioctl(handle, request, args);
    } while (-1 == ioctlStatus && EINTR == errno);
    return ioctlStatus;
}

int v4l2Device::GetAtomISPModes(enum AtomISPMode mode)
{
    switch(mode)
    {
        case VIDEO:    return _ISP_MODE_VIDEO;
        case PREVIEW:    return _ISP_MODE_PREVIEW;
        case CONTINUOUS:    return _ISP_MODE_CONTINUOUS;
        case STILL:    return _ISP_MODE_STILL;
        case NONE:

        default:
            return _ISP_MODE_NONE;
    }
}

int v4l2Device::ConvertToMFXFourCC(enum V4L2PixelFormat v4l2Format)
{
    switch (v4l2Format)
    {
        case UYVY:  return MFX_FOURCC_UYVY;
        case YUY2:  return MFX_FOURCC_YUY2;
        case NO_FORMAT:

        default:
            assert( !"Unsupported mfx fourcc");
            return 0;
    }
}

int v4l2Device::ConvertToV4L2FourCC()
{
    switch (m_v4l2Format)
    {
        case UYVY:    return V4L2_PIX_FMT_UYVY;
        case YUY2:    return V4L2_PIX_FMT_YUYV;
        case NO_FORMAT:

        default:
            assert( !"Unsupported v4l2 fourcc");
            return 0;
    }
}

void v4l2Device::Init( const char *devname,
            uint32_t width,
            uint32_t height,
            uint32_t num_buffers,
            enum V4L2PixelFormat v4l2Format,
            enum AtomISPMode MipiMode,
            int MipiPort)
{

    (devname != NULL)? m_devname = devname : m_devname;
    (m_width != width )? m_width = width : m_width;
    (m_height != height)? m_height = height : m_height;
    (m_num_buffers != num_buffers)? m_num_buffers = num_buffers : m_num_buffers;
    (m_v4l2Format != v4l2Format )? m_v4l2Format = v4l2Format : m_v4l2Format;
    (m_MipiMode != MipiMode )? m_MipiMode = MipiMode : m_MipiMode;
    (m_MipiPort != MipiPort )? m_MipiPort = MipiPort : m_MipiPort;

    memset(&m_format, 0, sizeof m_format);
    m_format.width = m_width;
    m_format.height = m_height;
    m_format.pixelformat = ConvertToV4L2FourCC();

    V4L2Init();
}

void v4l2Device::V4L2Init()
{
    int ret;
    struct v4l2_format fmt;
    struct v4l2_capability caps;
    struct v4l2_streamparm parm;
    struct v4l2_requestbuffers rqbufs;
    CLEAR(parm);

    m_fd = open(m_devname, O_RDWR);
    BYE_ON(m_fd < 0, "failed to open %s: %s\n", m_devname, ERRSTR);
    CLEAR(caps);

    /* Specifically for setting up mipi configuration. DMABUFF is
     * also enable by default here.
     */
    if (m_MipiPort > -1  && m_MipiMode != NONE) {
    parm.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    parm.parm.capture.capturemode = GetAtomISPModes(m_MipiMode);

    ret = blockIOCTL(m_fd, VIDIOC_S_INPUT, &m_MipiPort);
    BYE_ON(ret < 0, "VIDIOC_S_INPUT failed: %s\n", ERRSTR);

    ret = blockIOCTL(m_fd, VIDIOC_S_PARM, &parm);
    BYE_ON(ret < 0, "VIDIOC_S_PARAM failed: %s\n", ERRSTR);
    }

    ret = blockIOCTL(m_fd, VIDIOC_QUERYCAP, &caps);
    msdk_printf( "Driver Caps:\n"
            "  Driver: \"%s\"\n"
            "  Card: \"%s\"\n"
            "  Bus: \"%s\"\n"
            "  Version: %d.%d\n"
            "  Capabilities: %08x\n",
            caps.driver,
            caps.card,
            caps.bus_info,
            (caps.version>>16)&&0xff,
            (caps.version>>24)&&0xff,
            caps.capabilities);

    BYE_ON(ret, "VIDIOC_QUERYCAP failed: %s\n", ERRSTR);
    BYE_ON(~caps.capabilities & V4L2_CAP_VIDEO_CAPTURE,
                "video: singleplanar capture is not supported\n");

    CLEAR(fmt);
    fmt.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    ret = blockIOCTL(m_fd, VIDIOC_G_FMT, &fmt);

    BYE_ON(ret < 0, "VIDIOC_G_FMT failed: %s\n", ERRSTR);

    msdk_printf("G_FMT(start): width = %u, height = %u, 4cc = %.4s, BPP = %u sizeimage = %d field = %d\n",
        fmt.fmt.pix.width, fmt.fmt.pix.height,
        (char*)&fmt.fmt.pix.pixelformat,
        fmt.fmt.pix.bytesperline,
        fmt.fmt.pix.sizeimage,
        fmt.fmt.pix.field);

    fmt.fmt.pix = m_format;

    msdk_printf("G_FMT(pre): width = %u, height = %u, 4cc = %.4s, BPP = %u sizeimage = %d field = %d\n",
        fmt.fmt.pix.width, fmt.fmt.pix.height,
        (char*)&fmt.fmt.pix.pixelformat,
        fmt.fmt.pix.bytesperline,
        fmt.fmt.pix.sizeimage,
        fmt.fmt.pix.field);

    ret = blockIOCTL(m_fd, VIDIOC_S_FMT, &fmt);
    BYE_ON(ret < 0, "VIDIOC_S_FMT failed: %s\n", ERRSTR);

    ret = blockIOCTL(m_fd, VIDIOC_G_FMT, &fmt);
    BYE_ON(ret < 0, "VIDIOC_G_FMT failed: %s\n", ERRSTR);
    msdk_printf("G_FMT(final): width = %u, height = %u, 4cc = %.4s, BPP = %u\n",
        fmt.fmt.pix.width, fmt.fmt.pix.height,
        (char*)&fmt.fmt.pix.pixelformat,
        fmt.fmt.pix.bytesperline);

    CLEAR(rqbufs);
    rqbufs.count = m_num_buffers;
    rqbufs.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    rqbufs.memory = V4L2_MEMORY_DMABUF;

    ret = blockIOCTL(m_fd, VIDIOC_REQBUFS, &rqbufs);
    BYE_ON(ret < 0, "VIDIOC_REQBUFS failed: %s\n", ERRSTR);
    BYE_ON(rqbufs.count < m_num_buffers, "video node allocated only "
            "%u of %u buffers\n", rqbufs.count, m_num_buffers);

    m_format = fmt.fmt.pix;
}

void v4l2Device::V4L2Alloc()
{
    buffers = (Buffer *)malloc(sizeof(Buffer) * (int) m_num_buffers);
}

void v4l2Device::V4L2QueueBuffer(Buffer *buffer)
{
    struct v4l2_buffer buf;
    int ret;

    memset(&buf, 0, sizeof buf);
    buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    buf.memory = V4L2_MEMORY_DMABUF;
    buf.index = buffer->index;
    buf.m.fd = buffer->fd;

    ret = blockIOCTL(m_fd, VIDIOC_QBUF, &buf);
    BYE_ON(ret < 0, "VIDIOC_QBUF for buffer %d failed: %s (fd %u) (i %u)\n",
    buf.index, ERRSTR, buffer->fd, buffer->index);
}

Buffer *v4l2Device::V4L2DeQueueBuffer(Buffer *buffer)
{
    struct v4l2_buffer buf;
    int ret;

    memset(&buf, 0, sizeof buf);

    buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    buf.memory = V4L2_MEMORY_DMABUF;

    ret = blockIOCTL(m_fd, VIDIOC_DQBUF, &buf);
    BYE_ON(ret, "VIDIOC_DQBUF failed: %s\n", ERRSTR);

    return &buffer[buf.index];
}

void v4l2Device::V4L2StartCapture()
{
    int type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    int ret = 0;

    ret = blockIOCTL(m_fd, VIDIOC_STREAMON, &type);
    BYE_ON(ret < 0, "STREAMON failed: %s\n", ERRSTR);
}

void v4l2Device::V4L2StopCapture()
{
    int type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    int ret = 0;

    ret = blockIOCTL(m_fd, VIDIOC_STREAMOFF, &type);
    BYE_ON(ret < 0, "STREAMOFF failed: %s\n", ERRSTR);
}

void v4l2Device::PutOnQ(int x)
{
    pthread_mutex_lock(&mutex);
    m_q[m_first] = x;
    m_first = (m_first+1) % 5;
    m_numInQ++;
    pthread_mutex_unlock(&mutex);
    pthread_mutex_unlock(&empty);
}

int v4l2Device::GetOffQ()
{
    int thing;

    /* wait if the queue is empty. */
    while (m_numInQ == 0)
    pthread_mutex_lock(&empty);

    pthread_mutex_lock(&mutex);
    thing = m_q[m_last];
    m_last = (m_last+1) % 5;
    m_numInQ--;
    pthread_mutex_unlock(&mutex);

    return thing;
}

int v4l2Device::GetV4L2TerminationSignal()
{
    return (CtrlFlag && m_numInQ == 0)? 1 : 0;
}

static void CtrlCTerminationHandler(int s) { CtrlFlag = true; }

void *PollingThread(void *data)
{

    v4l2Device *v4l2 = (v4l2Device *)data;

    struct sigaction sigIntHandler;
    sigIntHandler.sa_handler = CtrlCTerminationHandler;
    sigemptyset(&sigIntHandler.sa_mask);
    sigIntHandler.sa_flags = 0;
    sigaction(SIGINT, &sigIntHandler, NULL);

    struct pollfd fd;
    fd.fd = v4l2->GetV4L2DisplayID();
    fd.events = POLLIN;

    while(1)
    {
        if (poll(&fd, 1, 5000) > 0)
        {
            if (fd.revents & POLLIN)
            {
                CurBuffers = v4l2->V4L2DeQueueBuffer(buffers);
                v4l2->PutOnQ(CurBuffers->index);

                if (CtrlFlag)
                    break;

                if (CurBuffers)
                    v4l2->V4L2QueueBuffer(&buffers[CurBuffers->index]);
            }
        }
    }
}

#endif // ifdef ENABLE_V4L2_SUPPORT
