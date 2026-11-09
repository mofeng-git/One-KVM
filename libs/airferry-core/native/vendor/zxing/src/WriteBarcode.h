/*
* Copyright 2024 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "Barcode.h"
#include "ImageView.h"

#include <memory>

namespace ZXing {

/**
 * @class WriterOptions
 * @brief Configuration options for barcode writing/generation.
 *
 * WriterOptions provides a fluent interface for setting various parameters
 * that control how barcodes are generated.
 *
 * This class supports method chaining for convenient option configuration.
 *
 * @example
 * auto opts = WriterOptions().scale(5).addHRT(true);
 */
class WriterOptions
{
	struct Data;

	std::unique_ptr<Data> d;

public:
	WriterOptions();
	~WriterOptions();
	WriterOptions(WriterOptions&&) noexcept;
	WriterOptions& operator=(WriterOptions&&) noexcept;

#define ZX_PROPERTY(TYPE, NAME) \
	TYPE NAME() const noexcept; \
	WriterOptions& NAME(TYPE v)&; \
	WriterOptions&& NAME(TYPE v)&&;

	/** @brief scale factor for rendering, i.e. the module size (default is 1)
	 * Passing a negative value will choose the scale automatically to fit the size of the barcode to abs(scale) as close as possible.
	 */
	ZX_PROPERTY(int, scale)

	/// rotate the barcode by given degrees (0, 90, 180, 270)
	ZX_PROPERTY(int, rotate)

	/// invert the colors of the barcode
	ZX_PROPERTY(bool, invert)

	/// add human readable text (HRI) to the barcode
	ZX_PROPERTY(bool, addHRT)

	/// add quiet zones around the barcode
	ZX_PROPERTY(bool, addQuietZones)

#undef ZX_PROPERTY
};


/**
 * Write barcode symbol to SVG
 *
 * @param barcode  Barcode to write
 * @param options  WriterOptions to parameterize rendering
 * @return std::string  SVG representation of barcode symbol
 */
std::string WriteBarcodeToSVG(const Barcode& barcode, const WriterOptions& options = {});

/**
 * Write barcode symbol to a utf8 string using graphical characters (e.g. '▀')
 *
 * @param barcode  Barcode to write
 * @param options  WriterOptions to parameterize rendering
 * @return std::string  Utf8 string representation of barcode symbol
 */
std::string WriteBarcodeToUtf8(const Barcode& barcode, const WriterOptions& options = {});

/**
 * Write barcode symbol to Image (Bitmap)
 *
 * @param barcode  Barcode to write
 * @param options  WriterOptions to parameterize rendering
 * @return Image  Bitmap representation of barcode symbol
 */
Image WriteBarcodeToImage(const Barcode& barcode, const WriterOptions& options = {});

} // ZXing
