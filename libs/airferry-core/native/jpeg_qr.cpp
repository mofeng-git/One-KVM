#include "airferry_zxing_core.h"
#include <turbojpeg.h>
#include <chrono>
#include <cstdint>
#include <vector>
#include <stdexcept>
#include <algorithm>
#include "BarcodeFormat.h"
#include "ImageView.h"
#include "ReadBarcode.h"

// A captured digital display has stable, high-contrast modules. Try the cheap
// histogram binarizer in known regions, retaining upstream's robust decoder as
// fallback for gradients, inverted codes, moved layouts or low-quality input.
static std::vector<AirFerryZxing::DecodeResult> DecodeTracked(
    const std::vector<uint8_t>& gray, int w, int h, const std::vector<int32_t>& hints) {
    std::vector<AirFerryZxing::DecodeResult> results;
    ZXing::ReaderOptions options;
    options.setFormats(ZXing::BarcodeFormat::QRCode);
    options.setTryHarder(false);
    options.setTryInvert(false);
    options.setBinarizer(ZXing::Binarizer::GlobalHistogram);
    const ZXing::ImageView full(gray.data(), w, h, ZXing::ImageFormat::Lum, w);
    for (size_t i = 0; i + 3 < hints.size(); i += 4) {
        const int margin = std::max(8, std::max(hints[i+2] - hints[i], hints[i+3] - hints[i+1]) * 8 / 100);
        const int x = std::clamp(hints[i] - margin, 0, w - 1);
        const int y = std::clamp(hints[i+1] - margin, 0, h - 1);
        const int right = std::clamp(hints[i+2] + margin, x + 1, w);
        const int bottom = std::clamp(hints[i+3] + margin, y + 1, h);
        const auto code = ZXing::ReadBarcode(full.cropped(x, y, right - x, bottom - y), options);
        if (!code.isValid() || code.bytes().empty()) continue;
        AirFerryZxing::Bbox bbox = {w, h, 0, 0};
        for (const auto& point : code.position()) {
            bbox[0] = std::min(bbox[0], static_cast<int>(point.x) + x);
            bbox[1] = std::min(bbox[1], static_cast<int>(point.y) + y);
            bbox[2] = std::max(bbox[2], static_cast<int>(point.x) + x);
            bbox[3] = std::max(bbox[3], static_cast<int>(point.y) + y);
        }
        results.push_back({std::vector<uint8_t>(code.bytes().begin(), code.bytes().end()), bbox});
    }
    return results;
}

struct Decoder {
    tjhandle jpeg = tjInitDecompress();
    std::vector<uint8_t> gray;
    std::vector<int32_t> hints;
    int width = 0, height = 0;
    unsigned tracked_frames = 0, frames = 0, full_resolution_frames = 0;
    int scale_num = 4;
    ~Decoder() { if (jpeg) tjDestroy(jpeg); }
};
using Clock = std::chrono::steady_clock;
using PayloadCallback = void (*)(const uint8_t*, size_t, void*);
extern "C" void* af_create() noexcept {
    try { return new Decoder; } catch (...) { return nullptr; }
}
extern "C" void af_destroy(void* decoder) noexcept { delete static_cast<Decoder*>(decoder); }
extern "C" int af_decode(void* decoder, const uint8_t* jpeg, size_t size,
                        PayloadCallback callback, void* context,
                        double* jpeg_ms, double* qr_ms) noexcept {
    try {
        auto& d = *static_cast<Decoder*>(decoder);
        if (!d.jpeg || !jpeg || !size) return -1;
        int w = 0, h = 0, subsamp = 0, colorspace = 0;
        if (tjDecompressHeader3(d.jpeg, jpeg, size, &w, &h, &subsamp, &colorspace) != 0 ||
            w <= 0 || h <= 0 || w > 4096 || h > 2160) return -1;
        const bool changed = d.width != w || d.height != h;
        if (changed) { d.hints.clear(); d.width = w; d.height = h; }
        // TurboJPEG's 3/4 IDCT avoids producing pixels that ZXing does not need.
        // Learn the full layout first; periodically reacquire at full resolution.
        int scale = w >= 1600 && h >= 900 && !changed &&
            d.full_resolution_frames == 0 && (++d.frames % 90 != 0) ? 3 : 4;
        if (d.full_resolution_frames) --d.full_resolution_frames;
        const size_t expected = d.hints.size() / 4;
        double jpeg_cost = 0, qr_cost = 0;
        auto decode = [&](int numerator) {
            const int sw = (w * numerator + 3) / 4, sh = (h * numerator + 3) / 4;
            if (d.scale_num != numerator) {
                for (auto& coordinate : d.hints) coordinate = coordinate * numerator / d.scale_num;
                d.scale_num = numerator;
            }
            d.gray.resize(static_cast<size_t>(sw) * sh);
            auto t0 = Clock::now();
            if (tjDecompress2(d.jpeg, jpeg, size, d.gray.data(), sw, sw, sh, TJPF_GRAY,
                              TJFLAG_FASTDCT) != 0) throw std::runtime_error("JPEG decode failed");
            auto t1 = Clock::now();
            std::vector<AirFerryZxing::DecodeResult> results;
            if (!d.hints.empty() && ++d.tracked_frames < 90) {
                results = DecodeTracked(d.gray, sw, sh, d.hints);
                if (results.size() < d.hints.size() / 4)
                    results = AirFerryZxing::DecodeMultiRegions(d.gray.data(), d.gray.size(),
                        sw, sh, sw, d.hints.data(), d.hints.size() / 4, 0.08f);
            }
            if (d.hints.empty() || d.tracked_frames >= 90 || results.size() < d.hints.size() / 4) {
                results = AirFerryZxing::DecodeMultiFull(d.gray.data(), d.gray.size(), sw, sh, sw);
                d.tracked_frames = 0;
            }
            jpeg_cost += std::chrono::duration<double, std::milli>(t1 - t0).count();
            qr_cost += std::chrono::duration<double, std::milli>(Clock::now() - t1).count();
            return results;
        };
        auto results = decode(scale);
        if (scale < 4 && (results.empty() || results.size() < expected)) {
            results = decode(4);
            // Small/dense QR layouts should stay at full resolution, avoiding
            // repeated failed attempts. Retry scaling only after 90 frames.
            d.full_resolution_frames = 90;
        }
        d.hints.clear();
        for (const auto& result : results)
            d.hints.insert(d.hints.end(), result.bbox.begin(), result.bbox.end());
        *jpeg_ms = jpeg_cost;
        *qr_ms = qr_cost;
        for (const auto& result : results) callback(result.payload.data(), result.payload.size(), context);
        return static_cast<int>(results.size());
    } catch (...) { return -2; }
}
