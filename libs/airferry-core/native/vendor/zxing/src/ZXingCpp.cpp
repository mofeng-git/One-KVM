/*
* Copyright 2024 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#include "ZXingCpp.h"

#include "Version.h"

namespace ZXing {

const std::string& Version()
{
	static std::string res = ZXING_VERSION_STR;
	return res;
}

} // namespace ZXing
