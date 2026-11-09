/*
* Copyright 2025 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "CharacterSet.h"
#include "ECI.h"
#include "Range.h"

#include <string>

namespace ZXing {

std::string BytesToUtf8(ByteView bytes, ECI eci);

inline std::string BytesToUtf8(ByteView bytes, CharacterSet cs)
{
	return BytesToUtf8(bytes, ToECI(cs));
}

} // ZXing
