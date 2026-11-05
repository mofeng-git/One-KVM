#include "rkmpp_dmabuf_ffi.h"
#include <array>
#include <cstdio>
#include <limits>
#include <new>
#include "rkmpp_dma_jpeg.h"

// Native MPP is already linked by the ARM FFmpeg/RKMPP build. Keep this
// optional for toolchains which only supply the FFmpeg headers.
#if defined(__linux__) && __has_include(<rockchip/rk_mpi.h>)
#define HAVE_MPP_DMA 1
#include <cerrno>
#include <linux/dma-buf.h>
#include <sys/ioctl.h>
extern "C" {
#include <rockchip/rk_mpi.h>
#include <rockchip/mpp_buffer.h>
#include <rockchip/mpp_frame.h>
#include <rockchip/mpp_packet.h>
#include <rockchip/mpp_task.h>
#include <rockchip/rk_venc_cfg.h>
#include <rockchip/rk_venc_rc.h>
}
#endif

static thread_local char dma_error[192] = {};
static int fail(const char *operation, int code) {
    std::snprintf(dma_error, sizeof(dma_error), "%s (ret=%d)", operation, code);
    return -1;
}

#ifdef HAVE_MPP_DMA
struct RkmppDmaEncoder {
    MppCtx ctx = nullptr;
    MppApi *api = nullptr;
    MppEncCfg cfg = nullptr;
    MppPacket packet = nullptr;
    std::array<MppBuffer, 16> buffers{};
    std::array<size_t, 16> capacities{};
    size_t count = 0;
    size_t minimum = 0;
    int width = 0, height = 0, stride = 0;
    MppFrameFormat format = MPP_FMT_YUV420SP;
    bool jpeg = false;
    MppCtx decoder = nullptr;
    MppApi *dec_api = nullptr;
    MppBufferGroup decoded_group = nullptr;
    MppBuffer decoded_buffer = nullptr;
    MppFrame decoded_frame = nullptr;
    MppPacket input_packet = nullptr;
    bool decoded_layout_set = false;
    int decoded_stride = 0, decoded_vstride = 0;

    void close() {
        if (packet) mpp_packet_deinit(&packet);
        if (ctx) {
            // A timeout must not expose still-in-use input to V4L2 QBUF.
            api->reset(ctx);
            mpp_destroy(ctx);
            ctx = nullptr;
        }
        // On any decoder failure, end hardware access before releasing the
        // input packet, exported buffers, or allowing the caller's QBUF.
        if (decoder) {
            dec_api->reset(decoder);
            mpp_destroy(decoder);
            decoder = nullptr;
        }
        if (input_packet) mpp_packet_deinit(&input_packet);
        if (decoded_frame) mpp_frame_deinit(&decoded_frame);
        if (decoded_buffer) { mpp_buffer_put(decoded_buffer); decoded_buffer = nullptr; }
        if (decoded_group) { mpp_buffer_group_put(decoded_group); decoded_group = nullptr; }
        for (auto &buffer : buffers) {
            if (buffer) { mpp_buffer_put(buffer); buffer = nullptr; }
        }
        if (cfg) { mpp_enc_cfg_deinit(cfg); cfg = nullptr; }
    }
    ~RkmppDmaEncoder() { close(); }
};

static bool set_cfg(RkmppDmaEncoder *e, const char *key, int value) {
    int ret = mpp_enc_cfg_set_s32(e->cfg, key, value);
    if (ret) fail(key, ret);
    return ret == 0;
}

extern "C" int rkmpp_dma_reconfigure(RkmppDmaEncoder *e, int kbps, int gop) {
    if (!e || !e->ctx || kbps <= 0 || kbps > 1000000 || gop <= 0)
        return fail("invalid DMA encoder configuration", -1);
    const int bps = kbps * 1000;
    if (!set_cfg(e, "rc:bps_target", bps) ||
        !set_cfg(e, "rc:bps_max", bps + bps / 16) ||
        !set_cfg(e, "rc:bps_min", bps - bps / 16) ||
        !set_cfg(e, "rc:gop", gop)) return -1;
    int ret = e->api->control(e->ctx, MPP_ENC_SET_CFG, e->cfg);
    return ret ? fail("MPP_ENC_SET_CFG", ret) : 0;
}

extern "C" RkmppDmaEncoder *rkmpp_dma_new(
    int width, int height, int stride, int format, int codec, int fps,
    int kbps, int gop, const int *fds, const size_t *sizes, size_t count) {
    if (width <= 0 || height <= 0 || width > 8192 || height > 8192 ||
        (width & 1) || (height & 1) || format < 0 || format > 4 ||
        codec < 0 || codec > 1 || fps <= 0 || fps > 240 ||
        (format != 4 && stride < width * (format == 1 || format == 3 ? 3 : format == 2 ? 2 : 1)) ||
        (format == 2 && stride % 16 != 0) || !fds || !sizes || !count || count > 16) {
        fail("invalid DMA frame layout", -1); return nullptr;
    }
    auto *e = new (std::nothrow) RkmppDmaEncoder;
    if (!e) { fail("allocate DMA encoder", -1); return nullptr; }
    e->width = width; e->height = height; e->stride = stride; e->count = count;
    e->jpeg = format == 4;
    if (e->jpeg) e->stride = stride = (width + 15) & ~15;
    const int vstride = e->jpeg ? (height + 15) & ~15 : height;
    e->format = format == 3 ? MPP_FMT_RGB888 : format == 2 ? MPP_FMT_YUV422_YUYV : format == 1 ? MPP_FMT_BGR888 : MPP_FMT_YUV420SP;
    auto abort_init = [e](const char *op, int ret) -> RkmppDmaEncoder * {
        fail(op, ret); delete e; return nullptr;
    };
    int ret = mpp_create(&e->ctx, &e->api);
    if (ret) return abort_init("mpp_create", ret);
    RK_S64 timeout = 2000;
    ret = e->api->control(e->ctx, MPP_SET_OUTPUT_TIMEOUT, &timeout);
    if (ret) return abort_init("MPP_SET_OUTPUT_TIMEOUT", ret);
    ret = e->api->control(e->ctx, MPP_SET_INPUT_TIMEOUT, &timeout);
    if (ret) return abort_init("MPP_SET_INPUT_TIMEOUT", ret);
    ret = mpp_init(e->ctx, MPP_CTX_ENC, codec ? MPP_VIDEO_CodingHEVC : MPP_VIDEO_CodingAVC);
    if (ret) return abort_init("mpp_init", ret);
    ret = mpp_enc_cfg_init(&e->cfg);
    if (ret) return abort_init("mpp_enc_cfg_init", ret);
    ret = e->api->control(e->ctx, MPP_ENC_GET_CFG, e->cfg);
    if (ret) return abort_init("MPP_ENC_GET_CFG", ret);
    if (!set_cfg(e, "prep:width", width) || !set_cfg(e, "prep:height", height) ||
        !set_cfg(e, "prep:hor_stride", stride) || !set_cfg(e, "prep:ver_stride", vstride) ||
        !set_cfg(e, "prep:format", e->format) || !set_cfg(e, "rc:mode", MPP_ENC_RC_MODE_CBR) ||
        !set_cfg(e, "rc:fps_in_flex", 0) || !set_cfg(e, "rc:fps_in_num", fps) ||
        !set_cfg(e, "rc:fps_in_denorm", 1) || !set_cfg(e, "rc:fps_out_flex", 0) ||
        !set_cfg(e, "rc:fps_out_num", fps) || !set_cfg(e, "rc:fps_out_denorm", 1) ||
        !set_cfg(e, "codec:type", codec ? MPP_VIDEO_CodingHEVC : MPP_VIDEO_CodingAVC)) {
        delete e; return nullptr;
    }
    // Match the browser-friendly baseline profile used by the existing RKMPP
    // byte encoder, rather than inheriting MPP's High-profile default.
    const int level = int64_t(width) * height * fps <= int64_t(1920) * 1080 * 60 ? 42 : 52;
    if (!codec && (!set_cfg(e, "h264:profile", 66) || !set_cfg(e, "h264:level", level) ||
                   !set_cfg(e, "h264:cabac_en", 0) || !set_cfg(e, "h264:trans8x8", 0))) {
        delete e; return nullptr;
    }
    if (rkmpp_dma_reconfigure(e, kbps, gop)) { delete e; return nullptr; }
    MppEncHeaderMode mode = MPP_ENC_HEADER_MODE_EACH_IDR;
    ret = e->api->control(e->ctx, MPP_ENC_SET_HEADER_MODE, &mode);
    if (ret) return abort_init("MPP_ENC_SET_HEADER_MODE", ret);
    // Reject arithmetic overflow even on 32-bit ARM; stride is supplied by a driver.
    if (size_t(stride) > std::numeric_limits<size_t>::max() / size_t(height))
        return abort_init("DMA buffer size overflow", -1);
    size_t minimum = size_t(stride) * height;
    if (format == 0) {
        if (minimum > std::numeric_limits<size_t>::max() / 3)
            return abort_init("DMA buffer size overflow", -1);
        minimum = minimum * 3 / 2;
    }
    if (e->jpeg) minimum = 68; // SOI/payload plus bounded hardware read headroom.
    e->minimum = minimum;
    for (size_t i = 0; i < count; ++i) {
        if (fds[i] < 0 || sizes[i] < minimum) return abort_init("short DMA buffer", -1);
        MppBufferInfo info{};
        info.type = MPP_BUFFER_TYPE_EXT_DMA; info.fd = fds[i];
        info.size = sizes[i]; info.index = static_cast<int>(i);
        ret = mpp_buffer_import(&e->buffers[i], &info);
        if (ret) return abort_init("mpp_buffer_import", ret);
        e->capacities[i] = sizes[i];
    }
    if (e->jpeg) {
        ret = mpp_create(&e->decoder, &e->dec_api);
        if (ret) return abort_init("mpp_create JPEG decoder", ret);
        ret = mpp_init(e->decoder, MPP_CTX_DEC, MPP_VIDEO_CodingMJPEG);
        if (ret) return abort_init("mpp_init JPEG decoder", ret);
        MppFrameFormat output = MPP_FMT_YUV420SP;
        ret = e->dec_api->control(e->decoder, MPP_DEC_SET_OUTPUT_FORMAT, &output);
        if (ret) return abort_init("JPEG NV12 output", ret);
        ret = mpp_buffer_group_get_internal(&e->decoded_group, MPP_BUFFER_TYPE_DRM);
        if (ret) return abort_init("JPEG output buffer group", ret);
        // MPP JPEG requires aligned storage; reserve the conservative size used
        // by its advanced-task decoder demo. One output reused after encode.
        ret = mpp_buffer_get(e->decoded_group, &e->decoded_buffer, size_t(stride) * vstride * 4);
        if (ret) return abort_init("JPEG output buffer", ret);
        ret = mpp_frame_init(&e->decoded_frame);
        if (ret) return abort_init("JPEG output frame", ret);
        mpp_frame_set_buffer(e->decoded_frame, e->decoded_buffer);
    }
    return e;
}

static int dma_read_sync(MppBuffer buffer, bool start) {
    dma_buf_sync sync{};
    sync.flags = DMA_BUF_SYNC_READ | (start ? DMA_BUF_SYNC_START : DMA_BUF_SYNC_END);
    int ret;
    do { ret = ioctl(mpp_buffer_get_fd(buffer), DMA_BUF_IOCTL_SYNC, &sync); }
    while (ret < 0 && errno == EINTR);
    return ret;
}

static int decode_jpeg(RkmppDmaEncoder *e, size_t index, size_t bytes_used) {
    MppBuffer input = e->buffers[index];
    // The parser reads only header bytes with explicit DMA CPU-read ownership.
    if (dma_read_sync(input, true)) return fail("JPEG DMA read sync start", errno);
    const auto *data = static_cast<const uint8_t *>(mpp_buffer_get_ptr(input));
    const bool valid = rkmpp_dma_jpeg_header(data, bytes_used, e->width, e->height);
    if (dma_read_sync(input, false)) return fail("JPEG DMA read sync end", errno);
    if (!valid) return fail("unsupported/mismatched JPEG header", -1);

    int ret = mpp_packet_init_with_buffer(&e->input_packet, input);
    if (ret) return fail("JPEG input packet", ret);
    mpp_packet_set_length(e->input_packet, bytes_used);
    MppTask task = nullptr;
    ret = e->dec_api->poll(e->decoder, MPP_PORT_INPUT, static_cast<MppPollType>(2000));
    if (ret) return fail("JPEG input poll", ret);
    ret = e->dec_api->dequeue(e->decoder, MPP_PORT_INPUT, &task);
    if (ret || !task) return fail("JPEG input task", ret);
    ret = mpp_task_meta_set_packet(task, KEY_INPUT_PACKET, e->input_packet);
    if (ret) return fail("JPEG input metadata", ret);
    ret = mpp_task_meta_set_frame(task, KEY_OUTPUT_FRAME, e->decoded_frame);
    if (ret) return fail("JPEG output metadata", ret);
    ret = e->dec_api->enqueue(e->decoder, MPP_PORT_INPUT, task);
    if (ret) return fail("JPEG submit", ret);
    task = nullptr;
    ret = e->dec_api->poll(e->decoder, MPP_PORT_OUTPUT, static_cast<MppPollType>(2000));
    if (ret) return fail("JPEG output poll", ret);
    ret = e->dec_api->dequeue(e->decoder, MPP_PORT_OUTPUT, &task);
    if (ret || !task) return fail("JPEG output task", ret);
    MppFrame result = nullptr;
    ret = mpp_task_meta_get_frame(task, KEY_OUTPUT_FRAME, &result);
    if (ret || result != e->decoded_frame) return fail("JPEG output frame mismatch", ret);
    ret = e->dec_api->enqueue(e->decoder, MPP_PORT_OUTPUT, task);
    if (ret) return fail("JPEG return output task", ret);
    ret = e->dec_api->poll(e->decoder, MPP_PORT_INPUT, static_cast<MppPollType>(2000));
    if (ret) return fail("JPEG input completion", ret);
    mpp_packet_deinit(&e->input_packet);
    if (mpp_frame_get_errinfo(result) || mpp_frame_get_discard(result) ||
        mpp_frame_get_info_change(result) ||
        mpp_frame_get_width(result) != unsigned(e->width) ||
        mpp_frame_get_height(result) != unsigned(e->height) ||
        mpp_frame_get_fmt(result) != MPP_FMT_YUV420SP ||
        mpp_frame_get_buffer(result) != e->decoded_buffer) {
        std::snprintf(dma_error, sizeof(dma_error),
            "invalid JPEG decoded frame: err=%u discard=%u info_change=%u size=%ux%u fmt=%x buffer_match=%d",
            mpp_frame_get_errinfo(result), mpp_frame_get_discard(result), mpp_frame_get_info_change(result),
            mpp_frame_get_width(result), mpp_frame_get_height(result), unsigned(mpp_frame_get_fmt(result)),
            int(mpp_frame_get_buffer(result) == e->decoded_buffer));
        return -1;
    }
    const int hs = mpp_frame_get_hor_stride(result), vs = mpp_frame_get_ver_stride(result);
    if (hs < e->width || vs < e->height || hs > 8192 || vs > 8192 || (hs & 15) || (vs & 15) ||
        size_t(hs) * vs * 3 / 2 > mpp_buffer_get_size(e->decoded_buffer))
        return fail("invalid JPEG decoded stride", -1);
    if (!e->decoded_layout_set) {
        if (!set_cfg(e, "prep:hor_stride", hs) || !set_cfg(e, "prep:ver_stride", vs)) return -1;
        ret = e->api->control(e->ctx, MPP_ENC_SET_CFG, e->cfg);
        if (ret) return fail("JPEG encoder layout", ret);
        e->decoded_stride = hs; e->decoded_vstride = vs; e->decoded_layout_set = true;
    } else if (hs != e->decoded_stride || vs != e->decoded_vstride) {
        return fail("JPEG decoded layout changed", -1);
    }
    return 0;
}

extern "C" int rkmpp_dma_encode(RkmppDmaEncoder *e, size_t index, size_t bytes_used, int fresh_fd, int64_t pts_us,
                                 int force_idr, const uint8_t **data, size_t *size) {
    if (!e || !e->ctx || index >= e->count || !data || !size)
        return fail("invalid DMA encode call", -1);
    *data = nullptr; *size = 0;
    if (e->packet) mpp_packet_deinit(&e->packet);
    auto abort_encode = [e](const char *op, int ret) {
        fail(op, ret); e->close(); return -1;
    };
    if (bytes_used > e->capacities[index] ||
        (e->jpeg ? (bytes_used < 4 || e->capacities[index] - bytes_used < 64) : bytes_used != e->minimum))
        return abort_encode("invalid DMA payload length", -1);
    if (fresh_fd >= 0) {
        MppBuffer replacement = nullptr;
        MppBufferInfo info{};
        info.type = MPP_BUFFER_TYPE_EXT_DMA; info.fd = fresh_fd;
        info.size = e->capacities[index]; info.index = static_cast<int>(index);
        const int ret = mpp_buffer_import(&replacement, &info);
        if (ret) return abort_encode("refresh USB DMA import", ret);
        if (e->buffers[index]) mpp_buffer_put(e->buffers[index]);
        e->buffers[index] = replacement;
    }
    if (e->jpeg && decode_jpeg(e, index, bytes_used)) {
        // Preserve the detailed decoder failure while ending BOTH engines.
        e->close(); return -1;
    }
    if (force_idr) {
        int ret = e->api->control(e->ctx, MPP_ENC_SET_IDR_FRAME, nullptr);
        if (ret) return abort_encode("MPP_ENC_SET_IDR_FRAME", ret);
    }
    MppFrame frame = e->jpeg ? e->decoded_frame : nullptr;
    int ret = 0;
    if (!e->jpeg) {
        ret = mpp_frame_init(&frame);
        if (ret) return abort_encode("mpp_frame_init", ret);
        mpp_frame_set_width(frame, e->width); mpp_frame_set_height(frame, e->height);
        mpp_frame_set_hor_stride(frame, e->stride); mpp_frame_set_ver_stride(frame, e->height);
        mpp_frame_set_fmt(frame, e->format);
        mpp_frame_set_buffer(frame, e->buffers[index]);
    }
    mpp_frame_set_pts(frame, pts_us);
    ret = e->api->encode_put_frame(e->ctx, frame);
    if (!e->jpeg) mpp_frame_deinit(&frame);
    if (ret) return abort_encode("encode_put_frame", ret);
    ret = e->api->encode_get_packet(e->ctx, &e->packet);
    if (ret || !e->packet) return abort_encode("encode_get_packet", ret);
    if (mpp_packet_is_partition(e->packet) || !mpp_packet_get_length(e->packet))
        return abort_encode("incomplete DMA encoder output", -1);
    // One synchronous input, no temporal scalability/reordering/split output.
    // A completed packet is the input-consumption barrier for this mode.
    *data = static_cast<const uint8_t *>(mpp_packet_get_pos(e->packet));
    *size = mpp_packet_get_length(e->packet);
    return 0;
}
extern "C" void rkmpp_dma_free(RkmppDmaEncoder *e) { delete e; }
#else
extern "C" RkmppDmaEncoder *rkmpp_dma_new(int,int,int,int,int,int,int,int,const int*,const size_t*,size_t) {
    fail("RKMPP DMA support not built", -1); return nullptr;
}
extern "C" int rkmpp_dma_encode(RkmppDmaEncoder*,size_t,size_t,int,int64_t,int,const uint8_t**,size_t*) { return -1; }
extern "C" int rkmpp_dma_reconfigure(RkmppDmaEncoder*,int,int) { return -1; }
extern "C" void rkmpp_dma_free(RkmppDmaEncoder*) {}
#endif
extern "C" const char *rkmpp_dma_error(void) { return dma_error; }
