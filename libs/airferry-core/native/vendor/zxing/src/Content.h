/*
* Copyright 2022 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "ByteArray.h"
#include "CharacterSet.h"
#include "ContentType.h"
#include "ECI.h"
#include "ReaderOptions.h"
#include "SymbologyIdentifier.h"
#include "ZXAlgorithms.h"

#if !defined(ZXING_READERS) && !defined(ZXING_WRITERS)
#include "Version.h"
#endif

#include <string>
#include <string_view>
#include <vector>

namespace ZXing {

class Content
{
	template <typename FUNC>
	void ForEachECIBlock(FUNC f) const;

	void switchEncoding(ECI eci, bool isECI);
	std::string render(bool withECI) const;

public:
	struct Encoding
	{
		ECI eci;
		int pos;
	};

	ByteArray bytes;
	std::vector<Encoding> encodings;
#if !defined(ZXING_READERS) && defined(ZXING_USE_ZINT)
	std::vector<std::string> utf8Cache;
#endif
	SymbologyIdentifier symbology;
	CharacterSet defaultCharset = CharacterSet::Unknown;
	bool hasECI = false;

	Content() = default;
	Content(ByteArray&& bytes, SymbologyIdentifier si, CharacterSet defaultCharset = CharacterSet::Unknown);

	// make movable but not copyable
	Content(const Content& other) = delete;
	Content& operator=(const Content& other) = delete;
	Content(Content&& other) = default;
	Content& operator=(Content&& other) = default;

	void switchEncoding(ECI eci) { switchEncoding(eci, true); }
	void switchEncoding(CharacterSet cs);

	void reserve(int count) { bytes.reserve(bytes.size() + count); }

	void push_back(uint8_t val) { bytes.push_back(val); }
	void push_back(int val) { bytes.push_back(narrow_cast<uint8_t>(val)); }
	void append(std::string_view str) { bytes.insert(bytes.end(), str.begin(), str.end()); }
	void append(ByteView bv) { bytes.insert(bytes.end(), bv.begin(), bv.end()); }
	void append(const Content& other);

	void erase(int pos, int n);
	void insert(int pos, std::string_view str);

	bool empty() const { return bytes.empty(); }
	bool canProcess() const;

	std::string text(TextMode mode) const;
	std::wstring utfW() const; // utf16 or utf32 depending on the platform, i.e. on size_of(wchar_t)
	std::string utf8() const { return render(false); }

	ByteArray bytesECI() const;
	CharacterSet guessEncoding() const;
	ContentType type() const;
};

} // ZXing
