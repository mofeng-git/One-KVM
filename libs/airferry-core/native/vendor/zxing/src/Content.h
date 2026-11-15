/*
* Copyright 2022 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "ByteArray.h"
#include "SymbologyIdentifier.h"
#include "ZXAlgorithms.h"

#include <string_view>
#include <utility>

namespace ZXing {

// One-KVM consumes raw QR payloads. Character-set interpretation and text
// rendering are intentionally absent; bytes are never transcoded or guessed.
class Content
{
public:
    ByteArray bytes;
    SymbologyIdentifier symbology;
    bool hasECI = false;

    Content() = default;
    Content(ByteArray&& data, SymbologyIdentifier si) : bytes(std::move(data)), symbology(si) {}
    Content(const Content&) = delete;
    Content& operator=(const Content&) = delete;
    Content(Content&&) = default;
    Content& operator=(Content&&) = default;

    void reserve(int count) { bytes.reserve(bytes.size() + count); }
    void push_back(uint8_t val) { bytes.push_back(val); }
    void push_back(int val) { bytes.push_back(narrow_cast<uint8_t>(val)); }
    void append(std::string_view str) { bytes.insert(bytes.end(), str.begin(), str.end()); }
    void append(const Content& other)
    {
        bytes.append(other.bytes);
        hasECI |= other.hasECI;
    }
    bool empty() const { return bytes.empty(); }
};

} // namespace ZXing
