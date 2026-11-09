#include "airferry_zxing_core.h"

#include <algorithm>
#include <cmath>
#include <limits>
#include <utility>

#include "BarcodeFormat.h"
#include "ImageView.h"
#include "ReadBarcode.h"

namespace AirFerryZxing {
namespace {

constexpr size_t kMaxTrackedCodes = 64;

ZXing::ReaderOptions ReaderOptions(bool try_invert = true)
{
    ZXing::ReaderOptions options;
    options.setFormats(ZXing::BarcodeFormat::QRCode);
    options.setTryHarder(true);
    options.setTryInvert(try_invert);
    return options;
}

template <typename Position>
Bbox PositionBbox(const Position& position, int32_t offset_x = 0, int32_t offset_y = 0)
{
    Bbox bbox = {
        std::numeric_limits<int32_t>::max(),
        std::numeric_limits<int32_t>::max(),
        std::numeric_limits<int32_t>::min(),
        std::numeric_limits<int32_t>::min(),
    };
    for (size_t i = 0; i < 4; ++i) {
        const int32_t x = static_cast<int32_t>(position[i].x) + offset_x;
        const int32_t y = static_cast<int32_t>(position[i].y) + offset_y;
        bbox[0] = std::min(bbox[0], x);
        bbox[1] = std::min(bbox[1], y);
        bbox[2] = std::max(bbox[2], x);
        bbox[3] = std::max(bbox[3], y);
    }
    return bbox;
}

std::optional<DecodeResult> ToResult(
    const ZXing::Barcode& barcode,
    int32_t offset_x = 0,
    int32_t offset_y = 0)
{
    if (!barcode.isValid()) {
        return std::nullopt;
    }
    const auto& bytes = barcode.bytes();
    if (bytes.empty()) {
        return std::nullopt;
    }
    return DecodeResult{
        std::vector<uint8_t>(bytes.begin(), bytes.end()),
        PositionBbox(barcode.position(), offset_x, offset_y),
    };
}

}  // namespace

bool ValidLuminanceGeometry(
    const uint8_t* pixels,
    size_t pixel_len,
    int32_t width,
    int32_t height,
    int32_t row_stride) noexcept
{
    if (pixels == nullptr || width <= 0 || height <= 0 || row_stride < width) {
        return false;
    }
    const uint64_t required =
        static_cast<uint64_t>(height - 1) * static_cast<uint64_t>(row_stride) +
        static_cast<uint64_t>(width);
    return required <= pixel_len;
}

std::vector<DecodeResult> DecodeMultiFull(
    const uint8_t* pixels,
    size_t pixel_len,
    int32_t width,
    int32_t height,
    int32_t row_stride)
{
    std::vector<DecodeResult> decoded;
    if (!ValidLuminanceGeometry(pixels, pixel_len, width, height, row_stride)) {
        return decoded;
    }
    const ZXing::ImageView view(pixels, width, height, ZXing::ImageFormat::Lum, row_stride);
    for (const auto& barcode : ZXing::ReadBarcodes(view, ReaderOptions())) {
        if (auto result = ToResult(barcode)) {
            decoded.push_back(std::move(*result));
            if (decoded.size() == kMaxTrackedCodes) {
                break;
            }
        }
    }
    return decoded;
}

std::vector<DecodeResult> DecodeMultiRegions(
    const uint8_t* pixels,
    size_t pixel_len,
    int32_t width,
    int32_t height,
    int32_t row_stride,
    const int32_t* hints,
    size_t hint_count,
    float margin_fraction)
{
    std::vector<DecodeResult> decoded;
    if (!ValidLuminanceGeometry(pixels, pixel_len, width, height, row_stride) ||
        hints == nullptr || hint_count == 0 || hint_count > kMaxTrackedCodes ||
        !std::isfinite(margin_fraction)) {
        return decoded;
    }

    margin_fraction = std::clamp(margin_fraction, 0.0F, 2.0F);
    const ZXing::ImageView full(pixels, width, height, ZXing::ImageFormat::Lum, row_stride);
    // Match Android's v1.1.3 JNI options for every tracked region.
    const ZXing::ReaderOptions options = ReaderOptions();
    decoded.reserve(hint_count);

    for (size_t i = 0; i < hint_count; ++i) {
        const int32_t* hint = hints + i * 4;
        const int32_t min_x = std::clamp(hint[0], 0, width);
        const int32_t min_y = std::clamp(hint[1], 0, height);
        const int32_t max_x = std::clamp(hint[2], 0, width);
        const int32_t max_y = std::clamp(hint[3], 0, height);
        if (max_x <= min_x || max_y <= min_y) {
            continue;
        }

        const int32_t qr_width = max_x - min_x;
        const int32_t qr_height = max_y - min_y;
        const int32_t qr_side = std::max(qr_width, qr_height);
        const int32_t margin = static_cast<int32_t>(std::min<double>(
            qr_side,
            static_cast<double>(qr_side) * margin_fraction));
        const int32_t expanded_x0 = std::max(0, min_x - margin);
        const int32_t expanded_y0 = std::max(0, min_y - margin);
        const int32_t expanded_x1 = std::min(width, max_x + margin);
        const int32_t expanded_y1 = std::min(height, max_y + margin);
        const int32_t side = std::min(
            expanded_x1 - expanded_x0,
            expanded_y1 - expanded_y0);
        if (side <= 0) {
            continue;
        }

        const int32_t center_x = (expanded_x0 + expanded_x1) / 2;
        const int32_t center_y = (expanded_y0 + expanded_y1) / 2;
        const int32_t x = std::clamp(center_x - side / 2, 0, width - side);
        const int32_t y = std::clamp(center_y - side / 2, 0, height - side);
        const ZXing::ImageView region = full.cropped(x, y, side, side);
        if (auto result = ToResult(ZXing::ReadBarcode(region, options), x, y)) {
            decoded.push_back(std::move(*result));
        }
    }
    return decoded;
}

}  // namespace AirFerryZxing
