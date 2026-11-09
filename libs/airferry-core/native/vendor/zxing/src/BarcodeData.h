/*
 * Copyright 2026 Axel Waggershauser
 */
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "Barcode.h"
#include "BitMatrix.h"
#include "Content.h"
#include "DecoderResult.h"
#include "DetectorResult.h"
#include "Error.h"
#include "ReaderOptions.h"
#include "StructuredAppend.h"

#include <memory>
#include <numbers>
#include <string>
#include <vector>

#ifdef ZXING_USE_ZINT
extern "C" struct zint_symbol;
struct zint_symbol_deleter
{
	void operator()(zint_symbol* p) const noexcept;
};
using unique_zint_symbol = std::unique_ptr<zint_symbol, zint_symbol_deleter>;
#else
struct zint_symbol {};
using unique_zint_symbol = std::unique_ptr<zint_symbol>;
#endif

namespace ZXing {

struct BarcodeData
{
	Content content;
	Error error = {};
	Position position = {};
	BarcodeFormat format = BarcodeFormat::None;
	std::string extra = {};
	StructuredAppendInfo sai = {};
	ReaderOptions readerOpts = {};
	BitMatrix symbol = {};
#ifdef ZXING_USE_ZINT
	unique_zint_symbol zint = {};
#endif
	int lineCount = 0;
	bool isMirrored = false;
	bool isInverted = false;

	bool operator==(const BarcodeData& other) const;

	inline bool isValid() const { return format != BarcodeFormat::None && !content.bytes.empty() && !error; }

	inline int orientation() const { return narrow_cast<int>(std::lround(position.orientation() * 180 / std::numbers::pi)); }
};

using BarcodesData = std::vector<BarcodeData>;

inline BarcodeData LinearBarcode(BarcodeFormat format, const std::string& text, int y, int xStart, int xStop, SymbologyIdentifier si,
                                 Error error = {}, std::string extra = {})
{
	return {.content = Content(ByteArray(text), si, CharacterSet::ISO8859_1),
			.error = std::move(error),
			.position = Line(y, xStart, xStop),
			.format = format,
			.extra = std::move(extra)};
}

inline BarcodeData MatrixBarcode(DecoderResult&& decodeResult, DetectorResult&& detectorResult, BarcodeFormat format)
{
	auto extra = std::move(decodeResult).json();
	if (JsonFind(extra, BarcodeExtra::Version).empty() && decodeResult.versionNumber())
		extra += JsonProp(BarcodeExtra::Version, std::to_string(decodeResult.versionNumber()));
	if (JsonFind(extra, BarcodeExtra::ECLevel).empty() && !decodeResult.ecLevel().empty())
		extra += JsonProp(BarcodeExtra::ECLevel, decodeResult.ecLevel());

	extra += JsonProp(BarcodeExtra::ReaderInit, decodeResult.readerInit());

	// the BitMatrix stores 'black'/foreground as 0xFF and 'white'/background as 0, but we
	// want the ImageView returned by symbol() to be a standard luminance image (black == 0)
	std::move(detectorResult).bits().flipAll();

	return {.content = std::move(decodeResult).content(),
			.error = std::move(decodeResult).error(),
			.position = std::move(detectorResult).position(),
			.format = format,
			.extra = std::move(extra),
			.sai = std::move(decodeResult).structuredAppend(),
			.symbol = std::move(detectorResult).bits(),
			.lineCount = decodeResult.lineCount(),
			.isMirrored = decodeResult.isMirrored()};
}

} // namespace ZXing
