/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
*/
// SPDX-License-Identifier: Apache-2.0

#include "Barcode.h"

#include "BarcodeData.h"
#include "JSON.h"
#include "ZXAlgorithms.h"

#include "BitMatrix.h"

#include <cmath>
#include <list>
#include <map>
#include <numbers>
#include <utility>

#ifdef ZXING_USE_ZINT
#include <zint.h>
void zint_symbol_deleter::operator()(zint_symbol* p) const noexcept
{
	ZBarcode_Delete(p);
}
#endif

namespace ZXing {

Barcode::Barcode() : d(std::make_shared<Data>()) {}

Barcode::Barcode(Data&& data) : d(std::make_shared<Data>(std::move(data))) {}

bool Barcode::isValid() const
{
	return d->isValid();
}

const Error& Barcode::error() const
{
	return d->error;
}

BarcodeFormat Barcode::format() const
{
	return d->format;
}

const Position& Barcode::position() const
{
	return d->position;
}

const std::vector<uint8_t>& Barcode::bytes() const
{
	return d->content.bytes;
}

std::vector<uint8_t> Barcode::bytesECI() const
{
	return d->content.bytesECI();
}

std::string Barcode::text(TextMode mode) const
{
	return d->content.text(mode);
}

std::string Barcode::text() const
{
	return text(d->readerOpts.textMode());
}

ContentType Barcode::contentType() const
{
	return d->content.type();
}

bool Barcode::hasECI() const
{
	return d->content.hasECI;
}

int Barcode::orientation() const
{
	return narrow_cast<int>(std::lround(d->position.orientation() * 180 / std::numbers::pi));
}

bool Barcode::isMirrored() const
{
	return d->isMirrored;
}

bool Barcode::isInverted() const
{
	return d->isInverted;
}

std::string Barcode::symbologyIdentifier() const
{
	return d->content.symbology.toString();
}

int Barcode::sequenceSize() const
{
	return d->sai.count;
}

int Barcode::sequenceIndex() const
{
	return d->sai.index;
}

std::string Barcode::sequenceId() const
{
	return d->sai.id;
}

int Barcode::lineCount() const
{
	return d->lineCount;
}

Barcode& Barcode::setReaderOptions(const ReaderOptions& opts)
{
	if (opts.characterSet() != CharacterSet::Unknown)
		d->content.defaultCharset = opts.characterSet();
	d->readerOpts = opts;
	return *this;
}

ImageView Barcode::symbol() const
{
	return !d->symbol.empty() ? ImageView{d->symbol.row(0).begin(), d->symbol.width(), d->symbol.height(), ImageFormat::Lum}
							  : ImageView{};
}

#if defined(ZXING_USE_ZINT) && defined(ZXING_EXPERIMENTAL_API)
zint_symbol* Barcode::zint() const
{
	return d->zint.get();
}
#endif

std::string Barcode::extra(std::string_view key) const
{
	if (key == "ALL") {
		if (format() == BarcodeFormat::None)
			return {};
		auto res =
			StrCat("{", JsonProp("Text", text(TextMode::Plain)), JsonProp("HRI", text(TextMode::HRI)),
				   JsonProp("TextECI", text(TextMode::ECI)), JsonProp("Bytes", text(TextMode::Hex)),
				   JsonProp("Identifier", symbologyIdentifier()), JsonProp("Type", Name(format())),
				   JsonProp("Symbology", Name(Symbology(format()))), JsonProp("ContentType", isValid() ? ToString(contentType()) : ""),
				   JsonProp("Position", ToString(position())), JsonProp("HasECI", hasECI()), JsonProp("IsMirrored", isMirrored()),
				   JsonProp("IsInverted", isInverted()), d->extra, JsonProp("Error", ToString(error())));
		res.back() = '}';
		return res;
	}
	return d->extra.empty() ? ""
		   : key.empty()    ? StrCat("{", std::string_view(d->extra).substr(0, d->extra.size() - 1), "}") // remove trailing ','
							: JsonGet<std::string>(d->extra, key).value_or(""); // make sure JsonUnescape() is called
}

bool Barcode::operator==(const Barcode& o) const
{
	return *d == *o.d;
}

bool BarcodeData::operator==(const BarcodeData& o) const
{
	if (format != o.format)
		return false;

	// handle MatrixCodes first
	if (!(format & BarcodeFormat::AllLinear)) {
		if (isValid() && o.isValid() && content.bytes != o.content.bytes)
			return false;

		// check for equal position if both are valid with equal bytes or at least one is in error
		return IsInside(Center(o.position), position);
	}

	if (content.bytes != o.content.bytes || error != o.error || orientation() != o.orientation())
		return false;

	if (lineCount > 1 && o.lineCount > 1)
		return HaveIntersectingBoundingBoxes(o.position, position);

	// the following code is only meant for this or other lineCount == 1
	assert(lineCount == 1 || o.lineCount == 1);

	// sl == single line, ml = multi line
	const auto& sl = lineCount == 1 ? *this : o;
	const auto& ml = lineCount == 1 ? o : *this;

	// If one line is less than half the length of the other away from the
	// latter, we consider it to belong to the same symbol.
	// Additionally, both need to have roughly the same length (see #367).
	auto dTop = maxAbsComponent(ml.position.topLeft() - sl.position.topLeft());
	auto dBot = maxAbsComponent(ml.position.bottomLeft() - sl.position.topLeft());
	auto slLength = maxAbsComponent(sl.position.topLeft() - sl.position.bottomRight());
	bool isHorizontal = sl.position.topLeft().y == sl.position.bottomRight().y;
	// Measure the multi line length in the same direction as the single line one (not diagonally)
	// to make sure overly tall symbols don't get segmented (see #769).
	auto mlLength = isHorizontal ? std::abs(ml.position.topLeft().x - ml.position.bottomRight().x)
								 : std::abs(ml.position.topLeft().y - ml.position.bottomRight().y);

	return std::min(dTop, dBot) < slLength / 2 && std::abs(slLength - mlLength) < slLength / 5;
}

Barcode MergeStructuredAppendSequence(const Barcodes& barcodes)
{
	if (barcodes.empty())
		return {};

	std::list<Barcode> allBarcodes(barcodes.begin(), barcodes.end());
	allBarcodes.sort([](const Barcode& r1, const Barcode& r2) { return r1.sequenceIndex() < r2.sequenceIndex(); });

	Barcode res = allBarcodes.front();
	for (auto i = std::next(allBarcodes.begin()); i != allBarcodes.end(); ++i)
		res.d->content.append(i->d->content);

	res.d->position = {};
	res.d->sai.index = -1;

	if (allBarcodes.back().sequenceSize() != Size(allBarcodes) ||
		!std::all_of(allBarcodes.begin(), allBarcodes.end(),
					 [&](Barcode& it) { return it.sequenceId() == allBarcodes.front().sequenceId(); }))
		res.d->error = FormatError("sequenceIDs not matching during structured append sequence merging");

	return res;
}

Barcodes MergeStructuredAppendSequences(const Barcodes& barcodes)
{
	std::map<std::string, Barcodes> sas;
	for (auto& barcode : barcodes) {
		if (barcode.isPartOfSequence())
			sas[barcode.sequenceId()].push_back(barcode);
	}

	Barcodes res;
	for (auto& [id, seq] : sas) {
		auto barcode = MergeStructuredAppendSequence(seq);
		if (barcode.isValid())
			res.push_back(std::move(barcode));
	}

	return res;
}

} // namespace ZXing
