/*
* Copyright 2024 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "Barcode.h"

#include <memory>
#include <optional>
#include <string_view>

extern "C" struct zint_symbol;

namespace ZXing {

/**
 * @class CreatorOptions
 * @brief Configuration options for barcode creation.
 *
 * This class encapsulates all the parameters needed to create a barcode with
 * specific format and settings.
 *
 * @details
 * The options property is a string that can contain multiple key-value pairs
 * separated by commas. Each key-value pair configures a specific aspect of the
 * barcode generation process and are dependent on the used BarcodeFormat.
 * Boolean properties are interpreted as true if only the property name is present.
 * Keys are case-insensitive. Passing a serialized JSON object is also supported.
 *
 * For a list of all supported options, see the list of read-only properties below.
 *
 * @example
 * auto opts = CreatorOptions(BarcodeFormat::QRCode, "ecLevel=30%, gs1");
 */
class CreatorOptions
{
	struct Data;

	std::unique_ptr<Data> d;

	friend Barcode CreateBarcode(const void* data, int size, int mode, const CreatorOptions& options);

public:
	CreatorOptions(BarcodeFormat format, std::string options = {});
	~CreatorOptions();
	CreatorOptions(CreatorOptions&&) noexcept;
	CreatorOptions& operator=(CreatorOptions&&) noexcept;

	zint_symbol* zint() const;

#define ZX_PROPERTY(TYPE, NAME) \
	const TYPE& NAME() const noexcept; \
	CreatorOptions& NAME(TYPE v)&; \
	CreatorOptions&& NAME(TYPE v)&&;

	ZX_PROPERTY(BarcodeFormat, format)
	ZX_PROPERTY(std::string, options)

#undef ZX_PROPERTY

#define ZX_RO_PROPERTY(TYPE, NAME) \
	std::optional<TYPE> NAME() const noexcept;

	ZX_RO_PROPERTY(std::string, ecLevel); // most 2D symbologies: ecLevel, e.g. "30%", see also libzint doc
	ZX_RO_PROPERTY(std::string, eci);     // most 2D symbologies: specify ECI designator to use
	ZX_RO_PROPERTY(bool, gs1);
	ZX_RO_PROPERTY(bool, readerInit);     // most 2D symbologies: set the "reader init" flag
	ZX_RO_PROPERTY(bool, forceSquare);    // DataMatrix: only consider square symbol versions
	ZX_RO_PROPERTY(int, columns);         // specify number of columns (e.g. for DataBarExpStk, PDF417)
	ZX_RO_PROPERTY(int, rows);            // specify number of rows (e.g. for DataBarExpStk, PDF417)
	ZX_RO_PROPERTY(int, version);         // most 2D symbologies: specify the version/size of the symbol
	ZX_RO_PROPERTY(int, dataMask);        // QRCode/MicroQRCode: specify dataMask to use

#undef ZX_RO_PROPERTY
};

/**
 * Generate barcode from unicode text
 *
 * @param contents  UTF-8 string to encode into a barcode
 * @param options  CreatorOptions (including BarcodeFormat)
 * @return #Barcode  generated barcode
 */
Barcode CreateBarcodeFromText(std::string_view contents, const CreatorOptions& options);

/**
 * Generate barcode from raw binary data
 *
 * @param data  array of bytes to encode into a barcode
 * @param size  size of byte array
 * @param options  CreatorOptions (including BarcodeFormat)
 * @return #Barcode  generated barcode
 */
Barcode CreateBarcodeFromBytes(const void* data, int size, const CreatorOptions& options);

#if defined(__cpp_lib_char8_t)
Barcode CreateBarcodeFromText(std::u8string_view contents, const CreatorOptions& options);
#endif

#if defined(__cpp_lib_ranges)
template <typename R>
requires std::ranges::contiguous_range<R> && std::ranges::sized_range<R> && (sizeof(std::ranges::range_value_t<R>) == 1)
Barcode CreateBarcodeFromBytes(const R& contents, const CreatorOptions& options)
{
	return CreateBarcodeFromBytes(std::ranges::data(contents), std::ranges::size(contents), options);
}
#else
template <typename R>
Barcode CreateBarcodeFromBytes(const R& contents, const CreatorOptions& options)
{
	return CreateBarcodeFromBytes(std::data(contents), std::size(contents), options);
}
#endif

} // ZXing
