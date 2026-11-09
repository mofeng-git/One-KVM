/*
* Copyright 2020 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "ODRowReader.h"
#include "Range.h"
#include "Pattern.h"

#include <array>
#include <cmath>
#include <numeric>

namespace ZXing::OneD::DataBar {

inline bool IsFinder(int a, int b, int c, int d, int e)
{
	//  a,b,c,d,e, g | sum(a..e) = 15
	//  ------------
	//  1,1,2
	//	| | |,1,1, 1
	//	3,8,9

	// use only pairs of bar+space to limit effect of poor threshold:
	// b+c can be 10, 11 or 12 modules, d+e is always 2
	int w = 2 * (b + c), n = d + e;
	// the offsets (5 and 2) are there to reduce quantization effects for small module sizes
	// TODO: review after switch to sub-pixel bar width calculation
	bool x = (w + 5 > 9 * n) &&
			 (w - 5 < 13 * n) &&
//			 (b < 5 + 9 * d) &&
//			 (c < 5 + 10 * e) &&
			 (a < 2 + 4 * e) &&
			 (4 * a > n);
#if defined(PRINT_DEBUG) && 0
	printf("[");
	for (bool v :
		 {w + 5 > 9 * n,
		  w - 5 < 13 * n,
//		  b < 5 + 9 * d,
//		  c < 5 + 10 * e,
		  a < 2 + 4 * e,
			4 * a > n})
		printf(" %d", v);
	printf("]"); fflush(stdout);
#endif
	return x;
};

inline PatternView Finder(const PatternView& view)
{
	return view.subView(8, 5);
}

inline PatternView LeftChar(const PatternView& view)
{
	return view.subView(0, 8);
}

inline PatternView RightChar(const PatternView& view)
{
	return view.subView(13, 8);
}

inline float ModSizeFinder(const PatternView& view)
{
	return Finder(view).sum() / 15.f;
}

inline bool IsGuard(int a, int b)
{
	//	printf(" (%d, %d)", a, b);
	return a > b * 3 / 4 - 2 && a < b * 5 / 4 + 2;
}

inline bool IsCharacter(const PatternView& view, int modules, float modSizeRef)
{
	float err = std::abs(float(view.sum()) / modules / modSizeRef - 1);
	return err < 0.1f;
}

struct Character
{
	int value = -1, checksum = 0;

	operator bool() const noexcept { return value != -1; }
	bool operator==(const Character& o) const noexcept { return value == o.value && checksum == o.checksum; }
	bool operator!=(const Character& o) const { return !(*this == o); }
};

struct Pair
{
	Character left, right;
	int finder = 0, xStart = -1, xStop = 1, y = -1, count = 1;

	operator bool() const noexcept { return finder != 0; }
	bool operator==(const Pair& o) const noexcept { return finder == o.finder && left == o.left && right == o.right; }
	bool operator!=(const Pair& o) const noexcept { return !(*this == o); }
	int center() const { return std::midpoint(xStart, xStop); }
};

struct PairHash
{
	std::size_t operator()(const Pair& p) const noexcept
	{
		return p.left.value ^ p.left.checksum ^ p.right.value ^ p.right.checksum ^ p.finder;
	}
};

constexpr int FULL_PAIR_SIZE = 8 + 5 + 8;
constexpr int HALF_PAIR_SIZE = 8 + 5 + 2; // half has to be followed by a guard pattern

template<int N>
int ParseFinderPattern(const PatternView& view, bool reversed, const std::array<std::array<int, 3>, N>& e2ePatterns)
{
	const auto e2e = NormalizedE2EPattern<5>(view, 15, reversed);

	int best_i = -1, best_e = 3;
	for (int i = 0; i < Size(e2ePatterns); ++i) {
		int e = 0;
		for (int j = 0; j < 3; ++j)
			e += std::abs(e2ePatterns[i][j] - e2e[j]);
		if (e < best_e) {
			best_e = e;
			best_i = i;
		}
	}
	int i = best_e <= 1 ? 1 + best_i : 0;
	return reversed ? -i : i;
}

template <typename T>
struct OddEven
{
	T odd = {}, evn = {};
	T& operator[](int i) { return i & 1 ? evn : odd; }
};

using Array4I = std::array<int, 4>;

// elements() determines the element widths of an (n,k) character.
// for DataBar:         LEN=8, mods=15/16
// for DataBarExpanded: LEN=8, mods=17
// for DataBarLimited:  LEN=14, mods=26/18
template <int LEN>
std::array<int, LEN> NormalizedPatternFromE2E(const PatternView& view, int mods, bool reversed = false)
{
	// To disambiguate the edge-to-edge measurements, it is defined that either the odd or the even-numbered
	// elements contain at least 1 element that is 1 module wide. (Note: even-numbered elements - 2nd, 4th, 6th, etc., have odd
	// indexes).
	// The reference decoding algorithm in Annex G of ISO/IEC 24724:2011 is distinguishing between DataBarExpanded on one side
	// and all other variants on the other. That seems to contradict the rest of the specification. For details see
	// https://github.com/zxing-cpp/zxing-cpp/issues/935.
	// Turns out the true distinction is to be made as follows:
	//   min-even-is-one: DataBarLimited, DataBar outside character
	//   min-odd-is-one:  DataBarExpanded, DataBar inside character
	bool minOddIsOne = mods == 15 || mods == 17;
	const auto e2e = NormalizedE2EPattern<LEN>(view, mods, reversed);
	std::array<int, LEN> widths;

	// derive element widths from normalized edge-to-similar-edge measurements
	int barSum = widths[0] = minOddIsOne ? 8 : 1; // first assume 1st bar is 1 / 8
	for (int i = 0; i < Size(e2e); i++) {
		widths[i + 1] = e2e[i] - widths[i];
		barSum += widths[i + 1];
	}
	widths.back() = mods - barSum; // last even element makes mods modules

	// int minEven = widths[1];
	// for (int i = 3; i < Size(widths); i += 2)
	// 	minEven = std::min(minEven, widths[i]);
	OddEven<int> min = {widths[0], widths[1]};
	for (int i = 2; i < Size(widths); i++)
		min[i] = std::min(min[i], widths[i]);

	if (minOddIsOne && min[0] > 1) {
		// minimum odd width is too big, readjust so minimum odd is 1
		for (int i = 0; i < Size(widths); i += 2) {
			widths[i] -= min[0] - 1;
			widths[i + 1] += min[0] - 1;
		}
	} else if (!minOddIsOne && min[1] > 1) {
		// minimum even width is too big, readjust so minimum even is 1
		for (int i = 0; i < Size(widths); i += 2) {
			widths[i] += min[1] - 1;
			widths[i + 1] -= min[1] - 1;
		}
	}

	return widths;
}

bool ReadDataCharacterRaw(const PatternView& view, int numModules, bool reversed, Array4I& oddPattern,
						  Array4I& evnPattern);

int GetValue(ArrayView<int> widths, int maxWidth, bool noNarrow);

Position EstimatePosition(const Pair& first, const Pair& last);
int EstimateLineCount(const Pair& first, const Pair& last);

} // namespace ZXing::OneD::DataBar
