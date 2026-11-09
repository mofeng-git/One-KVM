/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
* Copyright 2020 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "BarcodeFormat.h"
#include "CharacterSet.h"
#include "Version.h"

#include <string_view>
#include <utility>
#include <memory>

namespace ZXing {

/**
 * @brief The Binarizer enum
 *
 * Specify which algorithm to use for the grayscale to binary transformation.
 * The difference is how to get to a threshold value T which results in a bit
 * value R = L <= T.
 */
enum class Binarizer : unsigned char // needs to be unsigned for the bitfield below to work, uint8_t fails as well
{
	LocalAverage,    ///< T = average of neighboring pixels for matrix and GlobalHistogram for linear (HybridBinarizer)
	GlobalHistogram, ///< T = valley between the 2 largest peaks in the histogram (per line in linear case)
	FixedThreshold,  ///< T = 127
	BoolCast,        ///< T = 0, fastest possible
};

enum class EanAddOnSymbol : unsigned char // see above
{
	Ignore,  ///< Ignore any Add-On symbol during read/scan
	Read,    ///< Read EAN-2/EAN-5 Add-On symbol if found
	Require, ///< Require EAN-2/EAN-5 Add-On symbol to be present
};

enum class TextMode : unsigned char // see above
{
	Plain,   ///< bytes() transcoded to unicode based on ECI info or guessed charset (the default mode prior to 2.0)
	ECI,     ///< standard content following the ECI protocol with every character set ECI segment transcoded to unicode
	HRI,     ///< Human Readable Interpretation (dependent on the ContentType)
	Escaped, ///< Use the EscapeNonGraphical() function (e.g. ASCII 29 will be transcoded to "<GS>")
	Hex,     ///< bytes() transcoded to ASCII string of HEX values
	HexECI,  ///< bytesECI() transcoded to ASCII string of HEX values
};

/**
 * @class ReaderOptions
 * @brief Configuration options for barcode reading and decoding behavior.
 *
 * @details
 * ReaderOptions encapsulates a set of flags and parameters that control
 * how barcode detection and decoding is performed. It provides
 * fluent setters that support chaining. Both `name(val)` and `setName(val)`
 * forms are available for convenience and compatibility.
 *
 * The class is intended to be passed to the ReadBarcodes function to
 * influence scanning heuristics, performance vs. accuracy trade-offs, output
 * formatting, and symbol filtering. Instances can be reused across multiple
 * read operations.
 *
 * The default settings are optimized for detection rate and can be tuned
 * for speed or specific use-cases.
 *
 * @see BarcodeFormats, Binarizer, TextMode, CharacterSet, ReadBarcodes
 */
class ReaderOptions
{
	struct Data;
	std::unique_ptr<Data> d;

public:
	ReaderOptions();
	~ReaderOptions();
	ReaderOptions(const ReaderOptions&);
	ReaderOptions& operator=(const ReaderOptions&);
	ReaderOptions(ReaderOptions&&) noexcept;
	ReaderOptions& operator=(ReaderOptions&&) noexcept;

#define ZX_PROPERTY(TYPE, NAME, SETTER, ...) \
	TYPE NAME() const noexcept; \
	__VA_ARGS__ ReaderOptions& NAME(TYPE v) &; \
	__VA_ARGS__ ReaderOptions&& NAME(TYPE v) &&; \
	__VA_ARGS__ inline ReaderOptions& SETTER(TYPE v) & { return NAME(v); } \
	__VA_ARGS__ inline ReaderOptions&& SETTER(TYPE v) && { return std::move(*this).NAME(v); }

	/// Specify a set of BarcodeFormats that should be searched for, the default is all supported formats.
	const BarcodeFormats& formats() const noexcept;
	ReaderOptions& formats(BarcodeFormats&& v) &;
	ReaderOptions&& formats(BarcodeFormats&& v) &&;
	ReaderOptions& formats(const BarcodeFormats& v) & { return formats(BarcodeFormats(v)); }
	ReaderOptions&& formats(const BarcodeFormats& v) && { return std::move(*this).formats(BarcodeFormats(v)); }
	inline ReaderOptions& setFormats(BarcodeFormats&& v) & { return formats(std::move(v)); }
	inline ReaderOptions&& setFormats(BarcodeFormats&& v) && { return std::move(*this).formats(std::move(v)); }
	inline ReaderOptions& setFormats(const BarcodeFormats& v) & { return formats(BarcodeFormats(v)); }
	inline ReaderOptions&& setFormats(const BarcodeFormats& v) && { return std::move(*this).formats(BarcodeFormats(v)); }

	/// Spend more time to try to find a barcode; optimize for accuracy, not speed.
	ZX_PROPERTY(bool, tryHarder, setTryHarder)

	/// Also try detecting code in 90, 180 and 270 degree rotated images.
	ZX_PROPERTY(bool, tryRotate, setTryRotate)

	/// Also try detecting inverted ("reversed reflectance") codes if the format allows for those.
	ZX_PROPERTY(bool, tryInvert, setTryInvert)

	/// Also try detecting code in downscaled images (depending on image size).
	ZX_PROPERTY(bool, tryDownscale, setTryDownscale)

#ifdef ZXING_EXPERIMENTAL_API
	/// Also try detecting code after denoising (currently morphological closing filter for 2D formats only).
	ZX_PROPERTY(bool, tryDenoise, setTryDenoise)
#endif

	/// Binarizer to use internally when using the ReadBarcode function
	ZX_PROPERTY(Binarizer, binarizer, setBinarizer)

	/// Set to true if the input contains nothing but a single perfectly aligned barcode (generated image)
	ZX_PROPERTY(bool, isPure, setIsPure)

	/// Image size ( min(width, height) ) threshold at which to start downscaled scanning
	ZX_PROPERTY(uint16_t, downscaleThreshold, setDownscaleThreshold)

	/// Scale factor used during downscaling, meaningful values are 2, 3 and 4
	ZX_PROPERTY(uint8_t, downscaleFactor, setDownscaleFactor)

	/// The number of scan lines in a linear barcode that have to be equal to accept the result, default is 2
	ZX_PROPERTY(uint8_t, minLineCount, setMinLineCount)

	/// The maximum number of symbols (barcodes) to detect / look for in the image with ReadBarcodes
	ZX_PROPERTY(uint8_t, maxNumberOfSymbols, setMaxNumberOfSymbols)

	/// Validate optional checksums where applicable (e.g. Code39, ITF)
	ZX_PROPERTY(bool, validateOptionalChecksum, setValidateOptionalChecksum)

	/// If true, return the barcodes with errors as well (e.g. checksum errors, see @Barcode::error())
	ZX_PROPERTY(bool, returnErrors, setReturnErrors)

	/// Specify whether to ignore, read or require EAN-2/5 add-on symbols while scanning EAN/UPC codes
	ZX_PROPERTY(EanAddOnSymbol, eanAddOnSymbol, setEanAddOnSymbol)

	/// Specifies the TextMode that controls the return of the Barcode::text() function
	ZX_PROPERTY(TextMode, textMode, setTextMode)

	/// Specifies fallback character set to use instead of auto-detecting it (when applicable)
	ZX_PROPERTY(CharacterSet, characterSet, setCharacterSet)
	ReaderOptions& characterSet(std::string_view v) &;
	ReaderOptions&& characterSet(std::string_view v) &&;
	inline ReaderOptions& setCharacterSet(std::string_view v) & { return characterSet(v); }
	inline ReaderOptions&& setCharacterSet(std::string_view v) && { return std::move(*this).characterSet(v); }

#undef ZX_PROPERTY

	// Silence deprecated-declarations warnings, only happening here for deprecated inline functions and only with GCC
#ifdef __GNUC__
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

#define ZX_DEPRECATED_PROPERTY(TYPE, NAME, SETTER, GET_IMPL, SET_IMPL) \
	[[deprecated]] inline TYPE NAME() const noexcept { return GET_IMPL; } \
	[[deprecated]] ReaderOptions& NAME(TYPE v) & { SET_IMPL; return *this; } \
	[[deprecated]] ReaderOptions&& NAME(TYPE v) && { SET_IMPL; return std::move(*this); } \
	[[deprecated]] inline ReaderOptions& SETTER(TYPE v) & { return NAME(v); } \
	[[deprecated]] inline ReaderOptions&& SETTER(TYPE v) && { return std::move(*this).NAME(v); }

	/// Deprecated / does nothing. See BarcodeFormat::Code39Ext and ::Code39Std to select full ASCII or standard Code39 mode.
	ZX_DEPRECATED_PROPERTY(bool, tryCode39ExtendedMode, setTryCode39ExtendedMode, true, (void)v)

	/// Deprecated (use validateOptionalChecksum). The Code39 symbol has a valid checksum iff symbologyIdentifier()[2] is an odd digit
	ZX_DEPRECATED_PROPERTY(bool, validateCode39CheckSum, setValidateCode39CheckSum, validateOptionalChecksum(),
						   validateOptionalChecksum(v))

	/// Deprecated (use validateOptionalChecksum). The ITF symbol has a valid checksum iff symbologyIdentifier()[2] == '1'.
	ZX_DEPRECATED_PROPERTY(bool, validateITFCheckSum, setValidateITFCheckSum, validateOptionalChecksum(), validateOptionalChecksum(v))

#undef ZX_DEPRECATED_PROPERTY

#ifdef __GNUC__
#pragma GCC diagnostic pop
#endif

#ifdef ZXING_INTERNAL
	/// Check if a specific format is explicitly enabled in the formats set
	bool hasFormat(const BarcodeFormats& formats) const noexcept;

	/// Check if any format is explicitly or implicitly enabled in the formats set
	bool hasAnyFormat(const BarcodeFormats& formats) const noexcept;
#endif
};

} // ZXing
