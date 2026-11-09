/*
* Copyright 2025 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "DecoderResult.h"

#include <string>
#include <cstdint>
#include <vector>

namespace ZXing::Pdf417 {

struct PDF417CustomData : public CustomData
{
	std::string fileId;
	std::string sender;
	std::string addressee;
	std::string fileName;
	std::vector<int> optionalData;
	int64_t fileSize = -1;
	int64_t timestamp = -1;
	int checksum = -1;
	int approxSymbolWidth = -1;
	int segmentIndex = -1;
	int segmentCount = -1;
	bool isLastSegment = false;
};

} // namespace ZXing::Pdf417
