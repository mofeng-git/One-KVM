/*
* Copyright 2025 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#include "JSON.h"

#include "ZXAlgorithms.h"

#include <string_view>

// This code is trying to find the value of a key-value pair in a string of those.
// The input could be valid JSON, like '{"key": "val"}' or a stripped down version like
// 'key:val'. This is also compatible with the string serialization of a python dictionary.
// For convenience, also 'key=val' is supported, key is checked case insensitive and '_' is ignored.
// This could easily be done with the following regex (see below).
// But using std::regex adds 140k+ to the binary size. ctre is _very_ nice to use and has
// the same binary footprint as the hand-roled code below! But I don't want to add
// 5k lines of c++ code for that.

// #define ZXING_USE_CTRE
#ifdef ZXING_USE_CTRE
#pragma GCC diagnostic ignored "-Wundef" // see https://github.com/hanickadot/compile-time-regular-expressions/issues/340
#include "ctre.hpp"
#pragma GCC diagnostic error "-Wundef"

static constexpr auto PATTERN =
	ctll::fixed_string{R"(["']?([[:alpha:]][[:alnum:]]*)["']?\s*(?:[:]\s*["']?([[:alnum:]]+)["']?)?(?:,|\}|$))"};
#endif

namespace ZXing {

// Trim whitespace and quotes/braces from both ends
inline std::string_view Trim(std::string_view sv)
{
	return TrimWS(sv, " \t\n\r\"'{}");
}

inline bool IsEqualIgnoreCaseAndUnderscore(std::string_view a, std::string_view b)
{
	return IsEqualIgnoreCaseAnd(a, b, "_");
}

std::string_view JsonFind(std::string_view json, std::string_view key)
{
#ifdef ZXING_USE_CTRE
	for (auto [ma, mk, mv] : ctre::search_all<PATTERN>(json))
		if (IsEqualCaseInsensitive(key, mk))
			return mv.size() ? mv.to_view() : std::string_view(mk.data(), 0);

	return {};
#else
	json = Trim(json);

	// TODO: use something like ForEachToken here?
	while (!json.empty()) {
		auto posComma = json.find(',');
		auto pair = Trim(json.substr(0, posComma));

		if (IsEqualIgnoreCaseAndUnderscore(pair, key))
			return {pair.data(), 0}; // return non-null data to signal that we found the key

		auto posSep = pair.find_first_of(":=");

		if (posSep != std::string_view::npos && IsEqualIgnoreCaseAndUnderscore(Trim(pair.substr(0, posSep)), key))
			return Trim(pair.substr(posSep + 1));

		json = (posComma == std::string_view::npos) ? std::string_view{} : json.substr(posComma + 1);
	}

	return {};
#endif
}

std::string JsonEscapeStr(std::string_view str)
{
	std::string res;
	res.reserve(str.size() + 10);

	for (unsigned char c : str) {
		switch (c) {
		case '\"': res += "\\\""; break;
		case '\\': res += "\\\\"; break;
		case '\b': res += "\\b"; break;
		case '\f': res += "\\f"; break;
		case '\n': res += "\\n"; break;
		case '\r': res += "\\r"; break;
		case '\t': res += "\\t"; break;
		default:
			if (c <= 0x1F) {
				char buf[7];
				std::snprintf(buf, sizeof(buf), "\\u%04X", c);
				res += buf;
			} else {
				res += c;
			}
			break;
		}
	}

	return res;
}

std::string JsonUnEscapeStr(std::string_view str)
{
	std::string res;
	res.reserve(str.size());

	for (size_t i = 0; i < str.size(); ++i) {
		char c = str[i];
		if (c == '\\') {
			if (++i >= str.size())
				throw std::runtime_error("Invalid escape sequence");

			char esc = str[i];
			switch (esc) {
			case '"': res += '\"'; break;
			case '\\': res += '\\'; break;
			case '/': res += '/'; break;
			case 'b': res += '\b'; break;
			case 'f': res += '\f'; break;
			case 'n': res += '\n'; break;
			case 'r': res += '\r'; break;
			case 't': res += '\t'; break;
			case 'u': {
				if (i + 4 >= str.size())
					throw std::runtime_error("Incomplete \\u escape");

				uint32_t code = 0;
				auto first = str.data() + i + 1;
				auto last = first + 4;

				auto [ptr, ec] = std::from_chars(first, last, code, 16);
				if (ec != std::errc() || ptr != last)
					throw std::runtime_error("Failed to parse hex code");

				if (code > 0x1F)
					throw std::runtime_error("Unexpected code point in \\u escape");

				res += static_cast<char>(code);
				i += 4;
				break;
			}
			default: throw std::runtime_error("Unknown escape sequence");
			}
		} else {
			res += c;
		}
	}

	return res;
}

} // ZXing
