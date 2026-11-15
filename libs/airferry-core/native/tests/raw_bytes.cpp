// SPDX-License-Identifier: AGPL-3.0-only
#include "Barcode.h"
#include "DecoderResult.h"
#include "ReadBarcode.h"
#include "qrcode/QRVersion.h"

#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>
#include <utility>

namespace ZXing::QRCode {
DecoderResult DecodeBitStream(ByteArray&&, const Version&, ErrorCorrectionLevel);
}

namespace {
using namespace ZXing;

void Require(bool condition, const char* message)
{
    if (!condition)
        throw std::runtime_error(message);
}

struct Bits {
    ByteArray bytes;
    int size = 0;
    Bits& add(unsigned value, int count)
    {
        for (int i = count - 1; i >= 0; --i, ++size) {
            if (size % 8 == 0)
                bytes.push_back(0);
            bytes.back() |= ((value >> i) & 1) << (7 - size % 8);
        }
        return *this;
    }
    Bits& payload(const std::vector<uint8_t>& value)
    {
        for (auto b : value)
            add(b, 8);
        return *this;
    }
    DecoderResult decode(bool terminate = true)
    {
        if (terminate)
            add(0, 4);
        return QRCode::DecodeBitStream(std::move(bytes), *QRCode::Version::Model2(10), QRCode::ErrorCorrectionLevel::Low);
    }
};

void Check(Bits bits, const std::vector<uint8_t>& expected, bool eci = false)
{
    auto decoded = bits.decode();
    Require(decoded.isValid(), "valid bitstream rejected");
    Require(decoded.content().bytes == expected, "raw bytes changed");
    Require(decoded.content().hasECI == eci, "ECI presence lost");
}

void BitstreamCases()
{
    std::vector<uint8_t> allBytes;
    for (int i = 0; i < 256; ++i)
        allBytes.push_back(i);
    Check(Bits{}.add(4, 4).add(256, 16).payload(allBytes), allBytes);

    const std::vector<uint8_t> binary = {0, 0xff, 0x80, 0xc0, 0xaf, '\\', 0x1d, 0xe4, 0xb8, 0xad};
    // ECI changes interpretation of text, never the raw payload bytes.
    for (auto [assignment, count] : {std::pair{26u, 8}, {0x8000u | 300u, 16}, {0xc00000u | 100000u, 24}})
        Check(Bits{}.add(7, 4).add(assignment, count).add(4, 4).add(binary.size(), 16).payload(binary), binary, true);
    Check(Bits{}.add(7, 4).add(26, 8).add(4, 4).add(1, 16).add(0xff, 8)
                .add(7, 4).add(20, 8).add(4, 4).add(1, 16).add(0x80, 8), {0xff, 0x80}, true);

    // Non-byte modes still unpack their standard raw byte representation.
    Check(Bits{}.add(1, 4).add(4, 12).add(123, 10).add(4, 4), {'1', '2', '3', '4'});
    Check(Bits{}.add(2, 4).add(2, 11).add(10 * 45 + 11, 11), {'A', 'B'});
    Check(Bits{}.add(8, 4).add(1, 10).add(0, 13), {0x81, 0x40}); // Kanji / Shift-JIS
    Check(Bits{}.add(13, 4).add(1, 4).add(1, 10).add(0, 13), {0xa1, 0xa1}); // Hanzi / GB2312
    Check(Bits{}.add(5, 4).add(2, 4).add(1, 11).add(38, 6), {0x1d}); // FNC1 separator

    auto structured = Bits{}.add(3, 4).add(1, 4).add(2, 4).add(99, 8)
                            .add(4, 4).add(1, 16).add(0xff, 8).decode();
    Require(structured.isValid() && structured.content().bytes == std::vector<uint8_t>{0xff}, "structured payload changed");
    Require(structured.structuredAppend().index == 1 && structured.structuredAppend().count == 3, "structured metadata changed");

    Require(!Bits{}.add(7, 4).add(0xe0, 8).decode().isValid(), "invalid ECI prefix accepted");
    Require(!Bits{}.add(7, 4).add(0x80, 8).decode(false).isValid(), "truncated ECI accepted");
    Require(!Bits{}.add(4, 4).add(2, 16).add(0xff, 8).decode(false).isValid(), "truncated byte segment accepted");
    Require(!Bits{}.add(13, 4).add(2, 4).add(1, 10).add(0, 13).decode().isValid(), "unsupported Hanzi subset accepted");
}

void ImageCases()
{
    // Fixed QR Model 2 version 4/M, byte mode, independently generated with
    // Segno 1.6.6. No QR generator or character conversion library is needed.
    const std::vector<uint8_t> expected = {65, 105, 114, 70, 101, 114, 114, 121, 0, 255, 128, 192, 175, 92, 29, 228, 184, 173, 230, 150, 135, 230, 150, 135, 228, 187, 182, 46, 98, 105, 110};
    const char* rows[] = {
        "111111101010101000100001001111111",
        "100000101000100001000110001000001",
        "101110101101100011101011001011101",
        "101110100101100000000001001011101",
        "101110101000111101101001101011101",
        "100000100100000000001001001000001",
        "111111101010101010101010101111111",
        "000000000110010001111000100000000",
        "100111111001001110110110110010111",
        "010100010110001100001001110110001",
        "110010101001001101111100110110110",
        "111000010110101101011001101010101",
        "101010101100111011111111100010000",
        "000101001101000010010110011000110",
        "111101110110011011000101111011100",
        "100110000111100101110101011010111",
        "010110110110010001110010101010101",
        "011010011010100110100111000111100",
        "111011111110110110001000111101011",
        "011000000100000111100100101001111",
        "001101100111100100011100010110110",
        "100110011100111010101010111101001",
        "101100101011000110000111001111011",
        "100110011000011010000111100100011",
        "110010100001111100011011111111100",
        "000000001011100100100010100011111",
        "111111101010100000101110101010010",
        "100000101011001110000000100011100",
        "101110101010111001111100111111010",
        "101110101111001001001100011101110",
        "101110100011101011001110000010111",
        "100000100100010111100100100100111",
        "111111101110011001100011011011100",
    };
    constexpr int modules = 33, border = 4, scale = 8;
    constexpr int side = (modules + 2 * border) * scale;
    for (bool inverted : {false, true}) {
        for (bool rotated : {false, true}) {
            std::vector<uint8_t> pixels(side * side, inverted ? 0 : 255);
            for (int y = 0; y < modules * scale; ++y) {
                for (int x = 0; x < modules * scale; ++x) {
                    int px = x + border * scale, py = y + border * scale;
                    if (rotated)
                        std::swap(px, py);
                    pixels[py * side + px] = (rows[y / scale][x / scale] == '1') != inverted ? 0 : 255;
                }
            }
            ReaderOptions options;
            options.setFormats(BarcodeFormat::QRCode);
            options.setTryInvert(true);
            const auto barcode = ReadBarcode(ImageView(pixels.data(), side, side, ImageFormat::Lum), options);
            Require(barcode.isValid(), "QR image rejected");
            Require(barcode.bytes() == expected, "QR image binary payload changed");
        }
    }
}
}

int main()
{
    try {
        BitstreamCases();
        ImageCases();
        std::cout << "Raw QR byte regression cases passed\n";
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
