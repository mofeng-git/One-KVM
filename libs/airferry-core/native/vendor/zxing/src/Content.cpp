/*
 * Copyright 2022 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#include "Content.h"

#include "ByteArray.h"
#include "CharacterSet.h"
#include "ECI.h"
#include "HRI.h"
#include "TextDecoder.h"
#include "Utf.h"
#include "ZXAlgorithms.h"

#if !defined(ZXING_READERS) && !defined(ZXING_WRITERS)
#include "Version.h"
#endif

#include <cctype>

namespace ZXing {

std::string ToString(ContentType type)
{
	const char* t2s[] = {"Text", "Binary", "Mixed", "GS1", "ISO15434", "UnknownECI"};
	int idx = static_cast<int>(type);
	if (idx < 0 || idx >= Size(t2s))
		return "InvalidContentType";
	return t2s[idx];
}

template <typename FUNC>
void Content::ForEachECIBlock(FUNC func) const
{
	ECI defaultECI = hasECI ? ECI::ISO8859_1 : ECI::Unknown;
	if (encodings.empty())
		func(defaultECI, 0, Size(bytes));
	else if (encodings.front().pos != 0)
		func(defaultECI, 0, encodings.front().pos);

	for (int i = 0; i < Size(encodings); ++i) {
		auto [eci, start] = encodings[i];
		int end = i + 1 == Size(encodings) ? Size(bytes) : encodings[i + 1].pos;

		if (start != end)
			func(eci, start, end);
	}
}

void Content::switchEncoding(ECI eci, bool isECI)
{
	// remove all non-ECI entries on first ECI entry
	if (isECI && !hasECI)
		encodings.clear();
	if (isECI || !hasECI)
		encodings.push_back({eci, Size(bytes)});

	hasECI |= isECI;
}

Content::Content(ByteArray&& bytes, SymbologyIdentifier si, CharacterSet defaultCharset)
	: bytes(std::move(bytes)), symbology(si), defaultCharset(defaultCharset) {}

void Content::switchEncoding(CharacterSet cs)
{
	switchEncoding(ToECI(cs), false);
}

void Content::append(const Content& other)
{
	if (!hasECI && other.hasECI)
		encodings.clear();
	if (other.hasECI || !hasECI)
		for (auto& e : other.encodings)
			encodings.push_back({e.eci, Size(bytes) + e.pos});
	append(other.bytes);

	hasECI |= other.hasECI;
}

void Content::erase(int pos, int n)
{
	bytes.erase(bytes.begin() + pos, bytes.begin() + pos + n);
	for (auto& e : encodings)
		if (e.pos > pos)
			e.pos -= n;
}

void Content::insert(int pos, std::string_view str)
{
	bytes.insert(bytes.begin() + pos, str.begin(), str.end());
	for (auto& e : encodings)
		if (e.pos > pos)
			e.pos += Size(str);
}

bool Content::canProcess() const
{
	return std::all_of(encodings.begin(), encodings.end(), [](Encoding e) { return CanProcess(e.eci); });
}

std::string Content::render(bool withECI) const
{
	if (empty() || !canProcess())
		return {};

#ifdef ZXING_READERS
	std::string res;
	res.reserve(bytes.size() * 2);
	if (withECI)
		res += symbology.toString(true);
	ECI lastECI = ECI::Unknown;
	auto fallbackCS = defaultCharset;
	if (!hasECI && fallbackCS == CharacterSet::Unknown)
		fallbackCS = guessEncoding();

	ForEachECIBlock([&](ECI eci, int begin, int end) {
		// basic idea: if IsText(eci), we transcode it to UTF8, otherwise we treat it as binary but
		// transcoded it to valid UTF8 bytes sequences representing the code points 0-255. The eci we report
		// back to the caller by inserting their "\XXXXXX" ECI designator is UTF8 for text and
		// the original ECI for everything else.
		// first determine how to decode the content (use fallback if unknown)
		auto inEci = IsText(eci) ? eci : eci == ECI::Unknown ? ToECI(fallbackCS) : ECI::Binary;
		if (withECI) {
			// then find the eci to report back in the ECI designator
			auto outEci = IsText(inEci) ? ECI::UTF8 : eci;

			if (lastECI != outEci)
				res += ToString(outEci);
			lastECI = outEci;

			for (auto c : BytesToUtf8(bytes.asView(begin, end - begin), inEci)) {
				res += c;
				if (c == '\\') // in the ECI protocol a '\' (0x5c) has to be doubled, works only because 0x5c can only mean `\`
					res += c;
			}
		} else {
			res += BytesToUtf8(bytes.asView(begin, end - begin), inEci);
		}
	});

	return res;
#elif defined(ZXING_USE_ZINT)
	assert(!utf8Cache.empty());
	if (!withECI)
		return std::accumulate(utf8Cache.begin(), utf8Cache.end(), std::string());

	std::string res;
	res.reserve(3 + TransformReduce(utf8Cache, 0, std::size<std::string>) * 2 + encodings.size() * 7);
	res += symbology.toString(true);

	ECI lastECI = ECI::Unknown;
	auto fallbackCS = defaultCharset;
	if (!hasECI && fallbackCS == CharacterSet::Unknown)
		fallbackCS = guessEncoding();

	assert(utf8Cache.size() == encodings.size());
	for (int i = 0; i < Size(encodings); ++i) {
		const auto eci = encodings[i].eci;
		const auto inEci = IsText(eci) ? eci : eci == ECI::Unknown ? ToECI(fallbackCS) : ECI::Binary;
		const auto outEci = IsText(inEci) ? ECI::UTF8 : eci;

		if (lastECI != outEci)
			res += ToString(outEci);
		lastECI = outEci;

		for (auto c : utf8Cache[i]) {
			res += c;
			if (c == '\\') // in the ECI protocol a '\' (0x5c) has to be doubled, works only because 0x5c can only mean `\`
				res += c;
		}
	}

	return res;
#else
	(void)withECI;
	return std::string(bytes.asString());
#endif
}

std::string Content::text(TextMode mode) const
{
	switch (mode) {
	case TextMode::Plain: return render(false);
	case TextMode::ECI: return render(true);
	case TextMode::HRI:
		switch (type()) {
#if defined(ZXING_READERS) || defined(ZXING_USE_ZINT)
		case ContentType::GS1: {
			auto plain = render(false);
			auto hri = HRIFromGS1(plain);
			return hri.empty() ? plain : hri;
		}
		case ContentType::ISO15434: return HRIFromISO15434(render(false));
		case ContentType::Text: return render(false);
#endif
		default: return text(TextMode::Escaped);
		}
	case TextMode::Escaped: return EscapeNonGraphical(render(false));
	case TextMode::Hex: return ToHex(bytes);
	case TextMode::HexECI: return ToHex(bytesECI());
	}

	return {}; // silence compiler warning
}

std::wstring Content::utfW() const
{
	return FromUtf8(render(false));
}

ByteArray Content::bytesECI() const
{
	if (empty())
		return {};

	ByteArray res;
	res.reserve(3 + bytes.size() + hasECI * encodings.size() * 7);

	// report ECI protocol only if actually found ECI data in the barode bit stream
	// see also https://github.com/zxing-cpp/zxing-cpp/issues/936
	res.append(symbology.toString(hasECI));

	if (hasECI)
		ForEachECIBlock([&](ECI eci, int begin, int end) {
			if (hasECI)
				res.append(ToString(eci));

			for (auto b : bytes.asView(begin, end - begin)) {
				res.push_back(b);
				if (b == '\\') // in the ECI protocol a '\' has to be doubled
					res.push_back(b);
			}
		});
	else
		res.append(bytes);

	return res;
}

#if defined(ZXING_READERS) || defined(ZXING_USE_ZINT)
/**
* @param bytes bytes encoding a string, whose encoding should be guessed
* @return name of guessed encoding; at the moment will only guess one of:
*  {@link #SHIFT_JIS}, {@link #UTF8}, {@link #ISO88591}, or the platform
*  default encoding if none of these can possibly be correct
*/
CharacterSet GuessTextEncoding(ByteView bytes, CharacterSet fallback = CharacterSet::ISO8859_1)
{
	// For now, merely tries to distinguish ISO-8859-1, UTF-8 and Shift_JIS,
	// which should be by far the most common encodings.
	bool canBeISO88591 = true;
	bool canBeShiftJIS = true;
	bool canBeUTF8 = IsValidUtf8(bytes);
	int utf8BytesLeft = 0;
	//int utf8LowChars = 0;
	int utf2BytesChars = 0;
	int utf3BytesChars = 0;
	int utf4BytesChars = 0;
	int sjisBytesLeft = 0;
	//int sjisLowChars = 0;
	int sjisKatakanaChars = 0;
	//int sjisDoubleBytesChars = 0;
	int sjisCurKatakanaWordLength = 0;
	int sjisCurDoubleBytesWordLength = 0;
	int sjisMaxKatakanaWordLength = 0;
	int sjisMaxDoubleBytesWordLength = 0;
	//int isoLowChars = 0;
	//int isoHighChars = 0;
	int isoHighOther = 0;

	bool utf8bom = bytes.size() > 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF;

	for (int value : bytes)
	{
		if(!(canBeISO88591 || canBeShiftJIS || canBeUTF8))
			break;

		// UTF-8 stuff
		if (canBeUTF8) {
			if (utf8BytesLeft > 0) {
				if ((value & 0x80) == 0) {
					canBeUTF8 = false;
				}
				else {
					utf8BytesLeft--;
				}
			}
			else if ((value & 0x80) != 0) {
				if ((value & 0x40) == 0) {
					canBeUTF8 = false;
				}
				else {
					utf8BytesLeft++;
					if ((value & 0x20) == 0) {
						utf2BytesChars++;
					}
					else {
						utf8BytesLeft++;
						if ((value & 0x10) == 0) {
							utf3BytesChars++;
						}
						else {
							utf8BytesLeft++;
							if ((value & 0x08) == 0) {
								utf4BytesChars++;
							}
							else {
								canBeUTF8 = false;
							}
						}
					}
				}
			} //else {
			  //utf8LowChars++;
			  //}
		}

		// ISO-8859-1 stuff
		if (canBeISO88591) {
			if (value > 0x7F && value < 0xA0) {
				canBeISO88591 = false;
			}
			else if (value > 0x9F) {
				if (value < 0xC0 || value == 0xD7 || value == 0xF7) {
					isoHighOther++;
				} //else {
				  //isoHighChars++;
				  //}
			} //else {
			  //isoLowChars++;
			  //}
		}

		// Shift_JIS stuff
		if (canBeShiftJIS) {
			if (sjisBytesLeft > 0) {
				if (value < 0x40 || value == 0x7F || value > 0xFC) {
					canBeShiftJIS = false;
				}
				else {
					sjisBytesLeft--;
				}
			}
			else if (value == 0x80 || value == 0xA0 || value > 0xEF || (value < 0x20 && value != 0xa && value != 0xd)) {
				canBeShiftJIS = false; // use non-printable ASCII as indication for binary content
			}
			else if (value > 0xA0 && value < 0xE0) {
				sjisKatakanaChars++;
				sjisCurDoubleBytesWordLength = 0;
				sjisCurKatakanaWordLength++;
				if (sjisCurKatakanaWordLength > sjisMaxKatakanaWordLength) {
					sjisMaxKatakanaWordLength = sjisCurKatakanaWordLength;
				}
			}
			else if (value > 0x7F) {
				sjisBytesLeft++;
				//sjisDoubleBytesChars++;
				sjisCurKatakanaWordLength = 0;
				sjisCurDoubleBytesWordLength++;
				if (sjisCurDoubleBytesWordLength > sjisMaxDoubleBytesWordLength) {
					sjisMaxDoubleBytesWordLength = sjisCurDoubleBytesWordLength;
				}
			}
			else {
				//sjisLowChars++;
				sjisCurKatakanaWordLength = 0;
				sjisCurDoubleBytesWordLength = 0;
			}
		}
	}

	if (canBeUTF8 && utf8BytesLeft > 0) {
		canBeUTF8 = false;
	}
	if (canBeShiftJIS && sjisBytesLeft > 0) {
		canBeShiftJIS = false;
	}

	// Easy -- if there is BOM or at least 1 valid not-single byte character (and no evidence it can't be UTF-8), done
	if (canBeUTF8 && (utf8bom || utf2BytesChars + utf3BytesChars + utf4BytesChars > 0)) {
		return CharacterSet::UTF8;
	}

	bool assumeShiftJIS = fallback == CharacterSet::Shift_JIS || fallback == CharacterSet::EUC_JP;
	// Easy -- if assuming Shift_JIS or at least 3 valid consecutive not-ascii characters (and no evidence it can't be), done
	if (canBeShiftJIS && (assumeShiftJIS || sjisMaxKatakanaWordLength >= 3 || sjisMaxDoubleBytesWordLength >= 3)) {
		return CharacterSet::Shift_JIS;
	}
	// Distinguishing Shift_JIS and ISO-8859-1 can be a little tough for short words. The crude heuristic is:
	// - If we saw
	//   - only two consecutive katakana chars in the whole text, or
	//   - at least 10% of bytes that could be "upper" not-alphanumeric Latin1,
	// - then we conclude Shift_JIS, else ISO-8859-1
	if (canBeISO88591 && canBeShiftJIS) {
		return (sjisMaxKatakanaWordLength == 2 && sjisKatakanaChars == 2) || isoHighOther * 10 >= Size(bytes)
			? CharacterSet::Shift_JIS : CharacterSet::ISO8859_1;
	}

	// Otherwise, try in order ISO-8859-1, Shift JIS, UTF-8 and fall back to default platform encoding
	if (canBeISO88591) {
		return CharacterSet::ISO8859_1;
	}
	if (canBeShiftJIS) {
		return CharacterSet::Shift_JIS;
	}
	if (canBeUTF8) {
		return CharacterSet::UTF8;
	}
	// Otherwise, we take a wild guess with platform encoding
	return fallback;
}
#endif

CharacterSet Content::guessEncoding() const
{
#if defined(ZXING_READERS) || defined(ZXING_USE_ZINT)
	// assemble all blocks with unknown encoding
	ByteArray input;
	ForEachECIBlock([&](ECI eci, int begin, int end) {
		if (eci == ECI::Unknown)
			input.append(bytes.asView(begin, end - begin));
	});

	if (input.empty())
		return CharacterSet::Unknown;

	return GuessTextEncoding(input);
#else
	return CharacterSet::ISO8859_1;
#endif
}

ContentType Content::type() const
{
#if 1 //def ZXING_READERS
	if (empty())
		return ContentType::Text;

	if (!canProcess())
		return ContentType::UnknownECI;

	if (symbology.aiFlag == AIFlag::GS1)
		return ContentType::GS1;

	// check for the absolute minimum of a ISO 15434 conforming message ("[)>" + RS + digit + digit)
	if (bytes.size() > 6 && bytes.asString(0, 4) == "[)>\x1E" && std::isdigit(bytes[4]) && std::isdigit(bytes[5]))
		return ContentType::ISO15434;

	ECI fallback = ToECI(guessEncoding());
	std::vector<bool> binaryECIs;
	ForEachECIBlock([&](ECI eci, int begin, int end) {
		if (eci == ECI::Unknown)
			eci = fallback;
		binaryECIs.push_back((!IsText(eci)
							  || (ToInt(eci) > 0 && ToInt(eci) < 28 && ToInt(eci) != 25
								  && std::any_of(bytes.begin() + begin, bytes.begin() + end,
												 [](auto c) { return c < 0x20 && c != 0x9 && c != 0xa && c != 0xd; }))));
	});

	if (!Contains(binaryECIs, true))
		return ContentType::Text;
	if (!Contains(binaryECIs, false))
		return ContentType::Binary;

	return ContentType::Mixed;
#else
	//TODO: replace by proper construction from encoded data from within zint
	return ContentType::Text;
#endif
}

} // namespace ZXing
