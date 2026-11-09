/*
 * Copyright 2026 Axel Waggershauser
 */
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "Version.h"

#include <stdint.h>

// FLAGS[0]: l == linear, m == matrix
// FLAGS[1]: r == readable
// FLAGS[2]: w == writable (MultiformatWriter))
// FLAGS[3]: G == GS1
// FLAGS[4]: R == Retail, I == Industrial, O == Other

// clang-format off
//    NAME,            SYM, VAR, FLAGS, ZINT, ENABLED,                 HRI
#define ZX_BCF_LIST(X) \
	X(None,              0,   0,  "     ",   0, 1,                       "None") \
	X(All,              '*', '*', "     ",   0, 1,                       "All") \
	X(AllReadable,      '*', 'r', "     ",   0, 1,                       "All Readable") \
	X(AllCreatable,     '*', 'w', "     ",   0, 1,                       "All Creatable") \
	X(AllLinear,        '*', 'l', "     ",   0, 1,                       "All Linear") \
	X(AllMatrix,        '*', 'm', "     ",   0, 1,                       "All Matrix") \
	X(AllGS1,           '*', 'G', "     ",   0, 1,                       "All GS1") \
	X(AllRetail,        '*', 'R', "     ",   0, 1,                       "All Retail") \
	X(AllIndustrial,    '*', 'I', "     ",   0, 1,                       "All Industrial") \
	X(Codabar,          'F', ' ', "lrw  ",  18, ZXING_ENABLE_1D,         "Codabar") \
	X(Code39,           'A', ' ', "lrw I",   8, ZXING_ENABLE_1D,         "Code 39") \
	X(Code39Std,        'A', 's', "lrw I",   8, ZXING_ENABLE_1D,         "Code 39 Standard") \
	X(Code39Ext,        'A', 'e', "lr  I",   9, ZXING_ENABLE_1D,         "Code 39 Extended") \
	X(Code32,           'A', '2', "lr  I", 129, ZXING_ENABLE_1D,         "Code 32") \
	X(PZN,              'A', 'p', "lr  I",  52, ZXING_ENABLE_1D,         "Pharmazentralnummer") \
	X(Code93,           'G', ' ', "lrw I",  25, ZXING_ENABLE_1D,         "Code 93") \
	X(Code128,          'C', ' ', "lrwGI",  20, ZXING_ENABLE_1D,         "Code 128") \
	X(ITF,              'I', ' ', "lrw I",   3, ZXING_ENABLE_1D,         "ITF") \
	X(ITF14,            'I', '4', "lr  I",  89, ZXING_ENABLE_1D,         "ITF-14") \
	X(DataBar,          'e', ' ', "lr GR",  29, ZXING_ENABLE_1D,         "DataBar") \
	X(DataBarOmni,      'e', 'o', "lr GR",  29, ZXING_ENABLE_1D,         "DataBar Omni") \
	X(DataBarStk,       'e', 's', "lr GR",  79, ZXING_ENABLE_1D,         "DataBar Stacked") \
	X(DataBarStkOmni,   'e', 'O', "lr GR",  80, ZXING_ENABLE_1D,         "DataBar Stacked Omni") \
	X(DataBarLtd,       'e', 'l', "lr GR",  30, ZXING_ENABLE_1D,         "DataBar Limited") \
	X(DataBarExp,       'e', 'e', "lr GR",  31, ZXING_ENABLE_1D,         "DataBar Expanded") \
	X(DataBarExpStk,    'e', 'E', "lr GR",  81, ZXING_ENABLE_1D,         "DataBar Expanded Stacked") \
	X(EANUPC,           'E', ' ', "lr  R",  15, ZXING_ENABLE_1D,         "EAN/UPC") \
	X(EAN13,            'E', '1', "lrw R",  15, ZXING_ENABLE_1D,         "EAN-13") \
	X(EAN8,             'E', '8', "lrw R",  10, ZXING_ENABLE_1D,         "EAN-8") \
	X(EAN5,             'E', '5', "l   R",  12, ZXING_ENABLE_1D,         "EAN-5") \
	X(EAN2,             'E', '2', "l   R",  11, ZXING_ENABLE_1D,         "EAN-2") \
	X(ISBN,             'E', 'i', "lr  R",  69, ZXING_ENABLE_1D,         "ISBN") \
	X(UPCA,             'E', 'a', "lrw R",  34, ZXING_ENABLE_1D,         "UPC-A") \
	X(UPCE,             'E', 'e', "lrw R",  37, ZXING_ENABLE_1D,         "UPC-E") \
	X(OtherBarcode,     'X', ' ', " r   ",   0, ZXING_ENABLE_1D,         "Other barcode") /* see ISO/IEC 15424:2025 */ \
	X(DXFilmEdge,       'X', 'x', "lr   ", 147, ZXING_ENABLE_1D,         "DX Film Edge") \
	X(PDF417,           'L', ' ', "mrw  ",  55, ZXING_ENABLE_PDF417,     "PDF417") \
	X(CompactPDF417,    'L', 'c', "mr   ",  56, ZXING_ENABLE_PDF417,     "Compact PDF417") \
	X(MicroPDF417,      'L', 'm', "m    ",  84, ZXING_ENABLE_PDF417,     "MicroPDF417") \
	X(Aztec,            'z', ' ', "mr G ",  92, ZXING_ENABLE_AZTEC,      "Aztec") \
	X(AztecCode,        'z', 'c', "mrwG ",  92, ZXING_ENABLE_AZTEC,      "Aztec Code") \
	X(AztecRune,        'z', 'r', "mr   ", 128, ZXING_ENABLE_AZTEC,      "Aztec Rune") \
	X(QRCode,           'Q', ' ', "mrwG ",  58, ZXING_ENABLE_QRCODE,     "QR Code") \
	X(QRCodeModel1,     'Q', '1', "mr   ",   0, ZXING_ENABLE_QRCODE,     "QR Code Model 1") \
	X(QRCodeModel2,     'Q', '2', "mr   ",  58, ZXING_ENABLE_QRCODE,     "QR Code Model 2") \
	X(MicroQRCode,      'Q', 'm', "mr   ",  97, ZXING_ENABLE_QRCODE,     "Micro QR Code") \
	X(RMQRCode,         'Q', 'r', "mr G ", 145, ZXING_ENABLE_QRCODE,     "rMQR Code") \
	X(DataMatrix,       'd', ' ', "mrwG ",  71, ZXING_ENABLE_DATAMATRIX, "Data Matrix") \
	X(MaxiCode,         'U', ' ', "mr   ",  57, ZXING_ENABLE_MAXICODE,   "MaxiCode") \
	/* Add new formats here */
// clang-format on

#define ZX_BCF_ID(SYM, VAR) (((uint8_t)(SYM) << 0) | ((uint8_t)(VAR) << 8))

#ifdef __cplusplus

#include <array>
#include <algorithm>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace ZXing {

/**
* Enumerates barcode formats known to this package.
*/
enum class BarcodeFormat : unsigned int
{
#define X(NAME, SYM, VAR, FLAGS, ZINT, ENABLED, HRI) NAME = ZX_BCF_ID(SYM, VAR),
	ZX_BCF_LIST(X)
#undef X
	DataBarExpanded [[deprecated("Use DataBarExp instead")]] = DataBarExp,
	DataBarLimited [[deprecated("Use DataBarLtd instead")]] = DataBarLtd,
	LinearCodes [[deprecated("Use AllLinear instead")]] = AllLinear,
	MatrixCodes [[deprecated("Use AllMatrix instead")]] = AllMatrix,
	Any [[deprecated("Use All instead")]] = All,
};

inline char SymbologyKey(const BarcodeFormat& format)
{
	return uint32_t(format) & 0xFF;
}

inline char VariantKey(const BarcodeFormat& format)
{
	return (uint32_t(format) >> 8) & 0xFF;
}

inline std::string IdStr(const BarcodeFormat& format)
{
	return {']', SymbologyKey(format), VariantKey(format)};
}

/// @brief Returns the symbology (base type) of the given barcode format (e.g. EAN/UPC for EAN13, EAN8, UPCA, etc.).
BarcodeFormat Symbology(BarcodeFormat format);

/// @brief Returns the human-readable name of the given barcode format.
std::string_view Name(BarcodeFormat format);

/// Test if left hand side (e == element) is 'inside' right hand side (s == set) (e.g. MicroQRCode <= QRCode)
bool operator<=(BarcodeFormat e, BarcodeFormat s);

/// Test if the two BarcodeFormats have a non-empty intersection (e.g. AllMatrix & QRCode)
bool operator&(BarcodeFormat a, BarcodeFormat b);

template <std::size_t N>
using BarcodeFormatArray = std::array<BarcodeFormat, N>;

constexpr BarcodeFormatArray<2> operator|(BarcodeFormat a, BarcodeFormat b)
{
	return {a, b};
}

template <std::size_t N>
constexpr BarcodeFormatArray<N + 1> operator|(BarcodeFormatArray<N> bts, BarcodeFormat bt)
{
	BarcodeFormatArray<N + 1> ret{};
	for (std::size_t i = 0; i < N; ++i)
		ret[i] = bts[i];
	ret[N] = bt;
	return ret;
}

template <std::size_t N>
constexpr bool operator&(BarcodeFormat lhs, const BarcodeFormatArray<N>& rhs)
{
	return std::any_of(rhs.begin(), rhs.end(), [lhs](BarcodeFormat bt) { return lhs & bt; });
}

/**
 * @brief Parse a string into a BarcodeFormat. '-', '_', '/' and ' ' are optional.
 * @throws std::invalid_parameter if the string can not be fully parsed.
 */
BarcodeFormat BarcodeFormatFromString(std::string_view str);

std::string ToString(BarcodeFormat format);

/**
 * @class BarcodeFormats
 * @brief A small container representing a collection of BarcodeFormat values.
 *
 * BarcodeFormats encapsulates an ordered collection of BarcodeFormat values and
 * provides convenient construction, iteration, and set-like operations.
 *
 * General behavior and invariants:
 * - The internal representation is kept in a canonical form (sorted, no duplicates)
 *   using normalize().
 * - The API exposes read-only access and iteration.
 */
class BarcodeFormats
{
	std::vector<BarcodeFormat> formats_;

	void normalize();

public:
	BarcodeFormats() = default;
	BarcodeFormats(BarcodeFormat f) : formats_{f} {}

	/// @brief Constructs a collection from a compile-time array of BarcodeFormat values.
	/// @example BarcodeFormats formats = BarcodeFormat::QRCode | BarcodeFormat::EAN13;
	template <std::size_t N>
	BarcodeFormats(BarcodeFormatArray<N> formats) : formats_{formats.begin(), formats.end()}
	{
		normalize();
	}

	BarcodeFormats(std::vector<BarcodeFormat>&& formats) : formats_(std::move(formats)) { normalize(); }

	/// @brief Constructs a collection from a textual representation (e.g. a comma-separated list of format identifiers).
	explicit BarcodeFormats(std::string_view str);

	BarcodeFormats(const BarcodeFormats&) = default;
	BarcodeFormats(BarcodeFormats&&) = default;
	BarcodeFormats& operator=(const BarcodeFormats&) = default;
	BarcodeFormats& operator=(BarcodeFormats&&) = default;
	bool operator==(const BarcodeFormats& o) const noexcept { return formats_ == o.formats_; }
	bool operator!=(const BarcodeFormats& o) const noexcept { return !(*this == o); }
	// ~BarcodeFormats() = default;

	auto begin() const noexcept { return formats_.cbegin(); }
	auto end() const noexcept { return formats_.cend(); }

	using value_type = typename std::vector<BarcodeFormat>::value_type;
	using pointer = typename std::vector<BarcodeFormat>::pointer;
	auto data() const noexcept { return formats_.data(); }

	bool empty() const noexcept { return formats_.empty(); }
	int size() const noexcept { return static_cast<int>(formats_.size()); }

	// BarcodeFormats&& operator|(BarcodeFormat bt) &&;
	// BarcodeFormats&& operator|(const BarcodeFormats& other) &&;

	/// @brief Returns a BarcodeFormats containing the intersection of this collection and other.
	BarcodeFormats operator&(const BarcodeFormats& other);

	/// @brief Returns a list of available/supported barcode formats, optionally filtered by the provided formats.
	/// @example BarcodeFormats::list(BarcodeFormat::AllReadable);
	static BarcodeFormats list(const BarcodeFormats& filter = {});

	[[deprecated]] inline int count() const noexcept { return size(); }
	[[deprecated]] inline bool testFlag(BarcodeFormat format) const noexcept
	{
		return std::any_of(begin(), end(), [format](BarcodeFormat f) { return f & format; });
	}
	[[deprecated]] inline bool testFlags(const BarcodeFormats& formats) const noexcept
	{
		return std::any_of(begin(), end(), [formats](BarcodeFormat fo) {
			return std::any_of(formats.begin(), formats.end(), [fo](BarcodeFormat fi) { return fo & fi; });
		});
	}
};

inline bool operator<=(BarcodeFormat lhs, const BarcodeFormats& rhs)
{
	return std::any_of(rhs.begin(), rhs.end(), [lhs](BarcodeFormat bf) { return lhs <= bf; });
}

inline bool operator&(BarcodeFormat lhs, const BarcodeFormats& rhs)
{
	return std::any_of(rhs.begin(), rhs.end(), [lhs](BarcodeFormat bf) { return lhs & bf; });
}

/**
 * @brief Parse a string into a set of BarcodeFormats.
 * Separators can be (any combination of) '|' or ','.
 * Input can be lower case and any of '-', '_', '/' or ' ' are optional.
 * e.g. "EAN-8 | qrcode, Itf" would be parsed into [EAN8, QRCode, ITF].
 * @throws std::invalid_parameter if the string can not be fully parsed.
 */
BarcodeFormats BarcodeFormatsFromString(std::string_view str);

std::string ToString(const BarcodeFormats& formats);

} // namespace ZXing

#endif // __cplusplus
