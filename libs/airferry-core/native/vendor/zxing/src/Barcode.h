/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
* Copyright 2020 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "BarcodeFormat.h"
#include "ContentType.h"
#include "Error.h"
#include "ImageView.h"
#include "Quadrilateral.h"
#include "ReaderOptions.h" // for TextMode
#include "Version.h" // ZXING_... macros

#include <memory>
#include <string>
#include <vector>

#ifdef ZXING_USE_ZINT
extern "C" struct zint_symbol;
#endif

namespace ZXing {

class CreatorOptions;
class ReaderOptions;
class WriterOptions;
class Barcode;
struct BarcodeData;

using Position = QuadrilateralI;
using Barcodes = std::vector<Barcode>;

namespace BarcodeExtra {
	#define ZX_EXTRA(NAME) static constexpr auto NAME = #NAME
	ZX_EXTRA(DataMask); // QRCodes
	ZX_EXTRA(Version);
	ZX_EXTRA(EanAddOn); // EAN/UPC
	ZX_EXTRA(ECLevel);
	ZX_EXTRA(UPCE);
	ZX_EXTRA(ReaderInit);
	#undef ZX_EXTRA
} // namespace BarcodeExtra

/**
 * @brief The Barcode class encapsulates a decoded or created barcode symbol.
 */
class Barcode
{
	using Data = BarcodeData;

	std::shared_ptr<Data> d;

	Barcode& setReaderOptions(const ReaderOptions& opts);

	friend Barcode MergeStructuredAppendSequence(const Barcodes&);
	friend Barcodes ReadBarcodes(const ImageView&, const ReaderOptions&);
	friend Barcode CreateBarcode(const void*, int, int, const CreatorOptions&);
	friend Image WriteBarcodeToImage(const Barcode&, const WriterOptions&);
	friend std::string WriteBarcodeToSVG(const Barcode&, const WriterOptions&);

public:
	Barcode();
	Barcode(Barcode::Data&& data);

	bool isValid() const;

	const Error& error() const;

	/**
	 * @brief format returns the BarcodeFormat of the barcode
	 */
	BarcodeFormat format() const;

	/**
	 * @brief symbology returns the symbology of the barcode format (e.g. EAN/UPC for EAN13, EAN8, UPCA, etc.)
	 */
	BarcodeFormat symbology() const { return Symbology(format()); }

	/**
	 * @brief bytes is the raw / standard content without any modifications like character set conversions
	 */
	const std::vector<uint8_t>& bytes() const;

	/**
	 * @brief bytesECI is the raw / standard content following the ECI protocol
	 */
	std::vector<uint8_t> bytesECI() const;

	/**
	 * @brief text returns the bytes() content rendered to unicode/utf8 text according to specified TextMode
	 */
	std::string text(TextMode mode) const;

	/**
	 * @brief text returns the bytes() content rendered to unicode/utf8 text according to the TextMode set in the ReaderOptions
	 */
	std::string text() const;

	/**
	 * @brief contentType gives a hint to the type of content found (Text/Binary/GS1/etc.)
	 */
	ContentType contentType() const;

	/**
	 * @brief hasECI specifies whether or not an ECI tag was found
	 */
	bool hasECI() const;

	const Position& position() const;

	/**
	 * @brief orientation of barcode in degree, see also Position::orientation()
	 */
	int orientation() const;

	/**
	 * @brief isMirrored is the symbol mirrored (currently only supported by QRCode and DataMatrix)
	 */
	bool isMirrored() const;
	/**
	 * @brief isInverted is the symbol inverted / has reversed reflectance (see ReaderOptions::tryInvert)
	 */
	bool isInverted() const;

	/**
	 * @brief symbologyIdentifier Symbology identifier "]cm" where "c" is symbology code character, "m" the modifier.
	 */
	std::string symbologyIdentifier() const;

	/**
	 * @brief sequenceSize number of symbols in a structured append sequence.
	 *
	 * If this is not part of a structured append sequence, the returned value is -1.
	 * If it is a structured append symbol but the total number of symbols is unknown, the
	 * returned value is 0 (see PDF417 if optional "Segment Count" not given).
	 */
	int sequenceSize() const;

	/**
	 * @brief sequenceIndex the 0-based index of this symbol in a structured append sequence.
	 */
	int sequenceIndex() const;

	/**
	 * @brief sequenceId id to check if a set of symbols belongs to the same structured append sequence.
	 *
	 * If the symbology does not support this feature, the returned value is empty (see MaxiCode).
	 * For QR Code, this is the parity integer converted to a string.
	 * For PDF417 and DataMatrix, this is the "fileId".
	 */
	std::string sequenceId() const;

	bool isLastInSequence() const { return sequenceSize() == sequenceIndex() + 1; }
	bool isPartOfSequence() const { return sequenceSize() > -1 && sequenceIndex() > -1; }

	/**
	 * @brief Retrieve supplementary metadata associated with this barcode.
	 *
	 * Returns a string containing additional and symbology specific information. In form of a JSON object
	 * serialization. The optional parameter @p key can be used to retrieve a specific item only.
	 * Key values are case insensitive. See BarcodeExtra namespace for valid keys.
	 * If the key is not found or there is no info available, an empty string is returned.
	 */
	std::string extra(std::string_view key = "") const;

	/**
	 * @brief lineCount How many lines have been detected with this code (applies only to linear symbologies)
	 */
	int lineCount() const;

	/**
	 * @brief ecLevel returns the error correction level of the symbol (empty string if not applicable)
	 */
	// [[deprecated ("use extra(BarcodeExtra::ECLevel) instead")]]
	std::string ecLevel() const { return extra(BarcodeExtra::ECLevel); }

	/**
	 * @brief readerInit Set if Reader Initialisation/Programming symbol.
	 */
	// [[deprecated]]
	bool readerInit() const { return !extra(BarcodeExtra::ReaderInit).empty(); }

	/**
	 * @brief version QRCode / DataMatrix / Aztec version or size.
	 */
	// [[deprecated ("use extra(BarcodeExtra::Version) instead")]]
	std::string version() const { return extra(BarcodeExtra::Version); }

	ImageView symbol() const;
#if defined(ZXING_USE_ZINT) && defined(ZXING_EXPERIMENTAL_API)
	zint_symbol* zint() const;
#endif

	bool operator==(const Barcode& o) const;
};

/**
 * @brief Merge a list of Barcodes from one Structured Append sequence to a single barcode
 */
Barcode MergeStructuredAppendSequence(const Barcodes& barcodes);

/**
 * @brief Automatically merge all Structured Append sequences found in the given list of barcodes
 */
Barcodes MergeStructuredAppendSequences(const Barcodes& barcodes);

} // ZXing
