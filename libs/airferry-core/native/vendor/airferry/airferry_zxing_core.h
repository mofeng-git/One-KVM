#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <vector>

namespace AirFerryZxing {

using Bbox = std::array<int32_t, 4>;

struct DecodeResult {
    std::vector<uint8_t> payload;
    Bbox bbox;
};

bool ValidLuminanceGeometry(
    const uint8_t* pixels,
    size_t pixel_len,
    int32_t width,
    int32_t height,
    int32_t row_stride) noexcept;

std::vector<DecodeResult> DecodeMultiFull(
    const uint8_t* pixels,
    size_t pixel_len,
    int32_t width,
    int32_t height,
    int32_t row_stride);

std::vector<DecodeResult> DecodeMultiRegions(
    const uint8_t* pixels,
    size_t pixel_len,
    int32_t width,
    int32_t height,
    int32_t row_stride,
    const int32_t* hints,
    size_t hint_count,
    float margin_fraction);

}  // namespace AirFerryZxing
