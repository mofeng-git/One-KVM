/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
* Copyright 2020 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#include "ODReader.h"

#include "BinaryBitmap.h"
#include "ReaderOptions.h"
#include "ODCodabarReader.h"
#include "ODCode128Reader.h"
#include "ODCode39Reader.h"
#include "ODCode93Reader.h"
#include "ODDataBarExpandedReader.h"
#include "ODDataBarLimitedReader.h"
#include "ODDataBarReader.h"
#include "ODDXFilmEdgeReader.h"
#include "ODITFReader.h"
#include "ODMultiUPCEANReader.h"
#include "BarcodeData.h"

#include <algorithm>
#include <utility>

#ifdef PRINT_DEBUG
#include "BitMatrix.h"
#include "BitMatrixIO.h"
#endif

namespace ZXing::OneD {

Reader::Reader(const ReaderOptions& opts) : ZXing::Reader(opts)
{
	using enum BarcodeFormat;

	_readers.reserve(8);

	if (opts.hasAnyFormat(EANUPC))
		_readers.emplace_back(new MultiUPCEANReader(opts));

	if (opts.hasAnyFormat(Code39))
		_readers.emplace_back(new Code39Reader(opts));
	if (opts.hasAnyFormat(Code93))
		_readers.emplace_back(new Code93Reader(opts));
	if (opts.hasAnyFormat(Code128))
		_readers.emplace_back(new Code128Reader(opts));
	if (opts.hasAnyFormat(ITF))
		_readers.emplace_back(new ITFReader(opts));
	if (opts.hasAnyFormat(Codabar))
		_readers.emplace_back(new CodabarReader(opts));
	if (opts.hasFormat(DataBar | DataBarOmni | DataBarStk | DataBarStkOmni))
		_readers.emplace_back(new DataBarReader(opts));
	if (opts.hasFormat(DataBar | DataBarExp | DataBarExpStk))
		_readers.emplace_back(new DataBarExpandedReader(opts));
	if (opts.hasFormat(DataBar | DataBarLtd))
		_readers.emplace_back(new DataBarLimitedReader(opts));
	if (opts.hasAnyFormat(DXFilmEdge))
		_readers.emplace_back(new DXFilmEdgeReader(opts));
}

Reader::~Reader() = default;

/**
* We're going to examine rows from the middle outward, searching alternately above and below the
* middle, and farther out each time. rowStep is the number of rows between each successive
* attempt above and below the middle. So we'd scan row middle, then middle - rowStep, then
* middle + rowStep, then middle - (2 * rowStep), etc.
* rowStep is bigger as the image is taller, but is always at least 1. We've somewhat arbitrarily
* decided that moving up and down by about 1/16 of the image is pretty good; we try more of the
* image if "trying harder".
*/
BarcodesData DoDecode(const std::vector<std::unique_ptr<RowReader>>& readers, const BinaryBitmap& image, bool tryHarder,
						 bool rotate, bool isPure, int maxSymbols, int minLineCount, bool returnErrors)
{
	BarcodesData res;

	std::vector<std::unique_ptr<RowReader::DecodingState>> decodingState(readers.size());

	int width = image.width();
	int height = image.height();

	if (rotate)
		std::swap(width, height);

	int middle = height / 2;
	// TODO: find a better heuristic/parameterization if maxSymbols != 1
	int rowStep = std::max(1, height / ((tryHarder && !isPure) ? (maxSymbols == 1 ? 256 : 512) : 32));
	int maxLines = tryHarder ?
		height :	// Look at the whole image, not just the center
		15;			// 15 rows spaced 1/32 apart is roughly the middle half of the image

	if (isPure)
		minLineCount = 1;
	else
		minLineCount = std::min(minLineCount, height);
	std::vector<int> checkRows;

	PatternRow bars;
	bars.reserve(128); // e.g. EAN-13 has 59 bars/spaces

#ifdef PRINT_DEBUG
	BitMatrix dbg(width, height);
#endif

	for (int i = 0; i < maxLines; i++) {

		// Scanning from the middle out. Determine which row we're looking at next:
		int rowStepsAboveOrBelow = (i + 1) / 2;
		bool isAbove = (i & 0x01) == 0; // i.e. is x even?
		int rowNumber = middle + rowStep * (isAbove ? rowStepsAboveOrBelow : -rowStepsAboveOrBelow);
		bool isCheckRow = false;
		if (rowNumber < 0 || rowNumber >= height) {
			// Oops, if we run off the top or bottom, stop
			break;
		}

		// See if we have additional check rows (see below) to process
		if (checkRows.size()) {
			--i;
			rowNumber = checkRows.back();
			checkRows.pop_back();
			isCheckRow = true;
			if (rowNumber < 0 || rowNumber >= height)
				continue;
		}

		if (!image.getPatternRow(rowNumber, rotate ? 90 : 0, bars))
			continue;

#ifdef PRINT_DEBUG
		bool val = false;
		int x = 0;
		for (auto b : bars) {
			for(unsigned j = 0; j < b; ++j)
				dbg.set(x++, rowNumber, val);
			val = !val;
		}
#endif

		// While we have the image data in a PatternRow, it's fairly cheap to reverse it in place to
		// handle decoding upside down barcodes.
		// TODO: the DataBarExpanded (stacked) decoder depends on seeing each line from both directions. This is
		// 'surprising' and inconsistent. It also requires the decoderState to be shared between normal and reversed
		// scans, which makes no sense in general because it would mix partial detection data from two codes of the same
		// type next to each other. See also https://github.com/zxing-cpp/zxing-cpp/issues/87
		for (bool upsideDown : {false, true}) {
			// trying again?
			if (upsideDown) {
				// reverse the row and continue
				std::reverse(bars.begin(), bars.end());
			}
			// Look for a barcode
			for (size_t r = 0; r < readers.size(); ++r) {
				// If this is a pure symbol, then checking a single non-empty line is sufficient for all but the stacked
				// DataBar codes. They are the only ones using the decodingState, which we can use as a flag here.
				if (isPure && i && !decodingState[r])
					continue;

				PatternView next(bars);
				do {
					BarcodeData result = readers[r]->decodePattern(rowNumber, next, decodingState[r]);
					if (result.isValid() || (returnErrors && result.error)) {
						result.lineCount++;
						if (upsideDown) {
							// update position (flip horizontally).
							for (auto& p : result.position) {
								p = {width - p.x - 1, p.y};
							}
						}
						if (rotate) {
							for (auto& p : result.position) {
								p = {p.y, width - p.x - 1};
							}
						}

						// check if we know this code already
						for (auto& other : res) {
							if (result == other) {
								// merge the position information
								auto dTop = maxAbsComponent(other.position.topLeft() - result.position.topLeft());
								auto dBot = maxAbsComponent(other.position.bottomLeft() - result.position.topLeft());
								if (dTop < dBot || (dTop == dBot && rotate ^ (sumAbsComponent(other.position[0]) >
																			  sumAbsComponent(result.position[0])))) {
									other.position[0] = result.position[0];
									other.position[1] = result.position[1];
								} else {
									other.position[2] = result.position[2];
									other.position[3] = result.position[3];
								}
								other.lineCount++;
								// clear the result, so we don't insert it again below
								result = BarcodeData();
								break;
							}
						}

						if (result.format != BarcodeFormat::None) {
							res.push_back(std::move(result));

							// if we found a valid code we have not seen before but a minLineCount > 1,
							// add additional check rows above and below the current one
							if (!isCheckRow && minLineCount > 1 && rowStep > 1) {
								checkRows = {rowNumber - 1, rowNumber + 1};
								if (rowStep > 2)
									checkRows.insert(checkRows.end(), {rowNumber - 2, rowNumber + 2});
							}
						}

						if (maxSymbols && Reduce(res, 0, [&](int s, const BarcodeData& r) {
											  return s + (r.lineCount >= minLineCount);
										  }) == maxSymbols) {
							goto out;
						}
					}
					// make sure we make progress and we start the next try on a bar
					next.shift(2 - (next.index() % 2));
					next.extend();
				} while (tryHarder && next.size());
			}
		}
	}

out:
	// remove all symbols with insufficient line count
	std::erase_if(res, [&](auto&& r) { return r.lineCount < minLineCount; });

	// if symbols overlap, remove the one with a lower line count
	for (auto a = res.begin(); a != res.end(); ++a)
		for (auto b = std::next(a); b != res.end(); ++b)
			if (HaveIntersectingBoundingBoxes(a->position, b->position))
				*(a->lineCount < b->lineCount ? a : b) = BarcodeData();

	std::erase_if(res, [](auto&& r) { return r.format == BarcodeFormat::None; });

#ifdef PRINT_DEBUG
	SaveAsPBM(dbg, rotate ? "od-log-r.pnm" : "od-log.pnm");
#endif

	return res;
}

BarcodesData Reader::read(const BinaryBitmap& image, int maxSymbols) const
{
	auto resH =
		DoDecode(_readers, image, _opts.tryHarder(), false, _opts.isPure(), maxSymbols, _opts.minLineCount(), _opts.returnErrors());
	if ((!maxSymbols || Size(resH) < maxSymbols) && _opts.tryRotate()) {
		auto resV = DoDecode(_readers, image, _opts.tryHarder(), true, _opts.isPure(), maxSymbols - Size(resH),
								 _opts.minLineCount(), _opts.returnErrors());
		resH.insert(resH.end(), std::make_move_iterator(resV.begin()), std::make_move_iterator(resV.end()));
	}
	return resH;
}

} // namespace ZXing::OneD
