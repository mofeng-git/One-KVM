/*
* Copyright 2022 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "Range.h"

#include <string>
#include <string_view>

namespace ZXing {

bool IsValidUtf8(ByteView bytes);

std::string ToUtf8(std::wstring_view str);
std::wstring FromUtf8(std::string_view utf8);
#ifdef __cpp_lib_char8_t
std::wstring FromUtf8(std::u8string_view utf8);
#endif

std::wstring EscapeNonGraphical(std::wstring_view str);
std::string EscapeNonGraphical(std::string_view utf8);

} // namespace ZXing
