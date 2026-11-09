/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "ReaderOptions.h"
#include "BarcodeData.h"

namespace ZXing {

class BinaryBitmap;
class ReaderOptions;

class Reader
{
protected:
	const ReaderOptions& _opts;

public:
	const bool supportsInversion;

	explicit Reader(const ReaderOptions& opts, bool supportsInversion = false) : _opts(opts), supportsInversion(supportsInversion) {}
	explicit Reader(ReaderOptions&& opts) = delete;
	virtual ~Reader() = default;

	virtual BarcodesData read(const BinaryBitmap& image, int maxSymbols) const = 0;
};

} // ZXing
