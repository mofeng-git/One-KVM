/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
* Copyright 2020 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#include "ODCode93Reader.h"

#include "ODCode93Patterns.h"
#include "BarcodeData.h"
#include "SymbologyIdentifier.h"
#include "ZXAlgorithms.h"

#include <array>
#include <string>

namespace ZXing::OneD {

// Note that 'abcd' are dummy characters in place of control characters.
// Control chars ($)==a, (%)==b, (/)==c, (+)==d
static constexpr char ALPHABET[] = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%abcd*";

static_assert(Size(ALPHABET) == Size(Code93::CODE_PATTERNS), "table size mismatch");

static const int ASTERISK_ENCODING = 0x660; // E2E_PATTERNS[47]

static bool
CheckOneChecksum(const std::string& result, int checkPosition, int weightMax)
{
	int weight = 1;
	int checksum = 0;
	for (int i = checkPosition - 1; i >= 0; i--) {
		checksum += weight * IndexOf(ALPHABET, result[i]);
		if (++weight > weightMax) {
			weight = 1;
		}
	}
	return result[checkPosition] == ALPHABET[checksum % 47];
}

static bool
CheckChecksums(const std::string& result)
{
	int length = Size(result);
	return CheckOneChecksum(result, length - 2, 20) && CheckOneChecksum(result, length - 1, 15);
}

// forward declare here. see ODCode39Reader.cpp. Not put in header to not pollute the public facing API
std::string DecodeCode39AndCode93FullASCII(std::string encoded, const char ctrl[4]);

constexpr int CHAR_LEN = 6;
constexpr int CHAR_MODS = 9;
// quiet zone is half the width of a character symbol
constexpr float QUIET_ZONE_SCALE = 0.5f;

//TODO: make this a constexpr variable initialization
static auto E2E_PATTERNS = [ ] {
	// This creates an array of ints for fast IndexOf lookup of the edge-2-edge patterns (ISO/IEC 15417:2007(E) Table 2)
	// e.g. a code pattern of { 2, 1, 2, 2, 2, 2 } becomes the e2e pattern { 3, 3, 4, 4 } and the value 0bs100011110000.
	std::array<int, 48> res;
	for (int i = 0; i < Size(res); ++i) {
		const auto& a = Code93::CODE_PATTERNS[i];
		std::array<int, 4> e2e;
		for (int j = 0; j < 4; j++)
			e2e[j] = a[j] + a[j + 1];
		res[i] = ToInt(e2e);
	}
	return res;
}();

static bool IsStartGuard(const PatternView& window, int spaceInPixel)
{
	// The complete start pattern is FixedPattern<CHAR_LEN, CHAR_MODS>{1, 1, 1, 1, 4, 1}.
	// Use only the first 4 elements which results in more than a 2x speedup. This is counter-intuitive since we save at
	// most 1/3rd of the loop iterations in FindPattern. The reason might be a successful vectorization with the limited
	// pattern size that is missed otherwise. We check for the remaining 2 slots for plausibility of the 4:1 ratio.
	return IsPattern(window, FixedPattern<4, 4>{1, 1, 1, 1}, spaceInPixel, QUIET_ZONE_SCALE * 12) &&
		   window[4] > 3 * window[5] - 2 &&
		   ToInt(NormalizedE2EPattern<CHAR_LEN>(window, CHAR_MODS)) == ASTERISK_ENCODING;
}

BarcodeData Code93Reader::decodePattern(int rowNumber, PatternView& next, std::unique_ptr<DecodingState>&) const
{
	// minimal number of characters that must be present (including start, stop, checksum and 1 payload characters)
	int minCharCount = 5;

	next = FindLeftGuard<CHAR_LEN>(next, minCharCount * CHAR_LEN, IsStartGuard);
	if (!next.isValid())
		return {};

	int xStart = next.pixelsInFront();

	std::string txt;
	txt.reserve(20);

	do {
		// check remaining input width
		if (!next.skipSymbol())
			return {};

		txt += LookupBitPattern(ToInt(NormalizedE2EPattern<CHAR_LEN>(next, CHAR_MODS)), E2E_PATTERNS, ALPHABET);

		if (txt.back() == 0)
			return {};
	} while (txt.back() != '*');

	txt.pop_back(); // remove asterisk

	if (Size(txt) < minCharCount - 2)
		return {};

	// check termination bar (is present and not wider than about 2 modules) and quiet zone
	next = next.subView(0, CHAR_LEN + 1);
	if (!next.isValid() || next[CHAR_LEN] > next.sum(CHAR_LEN) / 4 || !next.hasQuietZoneAfter(QUIET_ZONE_SCALE))
		return {};

	Error error;
	if (!CheckChecksums(txt))
		error = ChecksumError();

	// Remove checksum digits
	txt.resize(txt.size() - 2);

	if (!error && (txt = DecodeCode39AndCode93FullASCII(txt, "abcd")).empty())
		error = FormatError("ASCII decoding of Code93 failed");

	// Symbology identifier ISO/IEC 15424:2008 4.4.10 no modifiers
	SymbologyIdentifier symbologyIdentifier = {'G', '0'};

	int xStop = next.pixelsTillEnd();
	return LinearBarcode(BarcodeFormat::Code93, txt, rowNumber, xStart, xStop, symbologyIdentifier, error);
}

} // namespace ZXing::OneD
