#pragma once
#include <stddef.h>
#include <stdint.h>

// Validate bounded JPEG headers before giving hardware a fixed-size output
// buffer. Only baseline 8-bit JPEG is accepted; other streams use copy fallback.
// No scan-data traversal or full-packet copy is needed.
inline bool rkmpp_dma_jpeg_header(const uint8_t *data, size_t size, int width, int height) {
    if (!data || size < 4 || data[0] != 0xff || data[1] != 0xd8) return false;
    size_t pos = 2;
    bool sof = false;
    while (pos < size) {
        if (data[pos++] != 0xff) return false;
        while (pos < size && data[pos] == 0xff) ++pos;
        if (pos == size) return false;
        const unsigned marker = data[pos++];
        if (!marker || marker == 0xd8 || marker == 0xd9 || marker == 1 ||
            (marker >= 0xd0 && marker <= 0xd7)) return false;
        if (size - pos < 2) return false;
        const size_t length = (size_t(data[pos]) << 8) | data[pos + 1];
        if (length < 2 || length > size - pos) return false;
        if (marker == 0xc0) {
            if (sof || length < 8 || data[pos + 2] != 8) return false;
            const unsigned h = (unsigned(data[pos + 3]) << 8) | data[pos + 4];
            const unsigned w = (unsigned(data[pos + 5]) << 8) | data[pos + 6];
            const unsigned components = data[pos + 7];
            if (w != unsigned(width) || h != unsigned(height) ||
                (components != 1 && components != 3) || length != 8 + 3 * components) return false;
            sof = true;
        } else if (marker >= 0xc0 && marker <= 0xcf && marker != 0xc4 && marker != 0xcc) {
            return false; // Progressive, lossless, extended or differential SOF.
        } else if (marker == 0xda) {
            return sof && length >= 6 && size - pos > length;
        }
        pos += length;
    }
    return false;
}
