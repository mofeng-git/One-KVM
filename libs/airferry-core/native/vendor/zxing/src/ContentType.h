/*
* Copyright 2026 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include <string>

namespace ZXing {

enum class ContentType { Text, Binary, Mixed, GS1, ISO15434, UnknownECI };

std::string ToString(ContentType type);

} // ZXing
