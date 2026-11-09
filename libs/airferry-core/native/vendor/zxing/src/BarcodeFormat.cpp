/*
 * Copyright 2026 Axel Waggershauser
 */
// SPDX-License-Identifier: Apache-2.0

#include "BarcodeFormat.h"

#include "Version.h"
#include "ZXAlgorithms.h"

#include <algorithm>

namespace ZXing {

// =================================== BarcodeFormat =================================

static_assert(ZX_BCF_ID('\0', '\0') == 0, "BarcodeFormat encoding error");

BarcodeFormat Symbology(BarcodeFormat format)
{
	return BarcodeFormat(ZX_BCF_ID(SymbologyKey(format), ' ' * (VariantKey(format) != 0)));
}

std::string_view Name(BarcodeFormat format)
{
	switch (format) {
#define X(NAME, SYM, VAR, FLAGS, ZINT, ENABLED, HRI) \
	case BarcodeFormat(ZX_BCF_ID(SYM, VAR)): return HRI;
		ZX_BCF_LIST(X)
#undef X
	default: return "Unknown";
	};
}

BarcodeFormat BarcodeFormatFromString(std::string_view str)
{
	if (str.size() < 3)
		throw std::invalid_argument(StrCat("This is not a valid barcode format: '", str, "'"));
#define X(NAME, SYM, VAR, FLAGS, ZINT, ENABLED, HRI) \
	if ((str[0] == ']' && str[1] == SYM && str[2] == VAR) || IsEqualIgnoreCase(str, #NAME) || IsEqualIgnoreCaseAnd(str, HRI, " -_/")) \
		return BarcodeFormat(ZX_BCF_ID(SYM, VAR));
	ZX_BCF_LIST(X)
#undef X
	throw std::invalid_argument(StrCat("This is not a valid barcode format: '", str, "'"));
}

std::string ToString(BarcodeFormat format)
{
	return std::string(Name(format));
}

bool operator<=(BarcodeFormat e, BarcodeFormat s)
{
	return e == s || s == BarcodeFormat::All
		   || (SymbologyKey(s) == '*' ? SymbologyKey(e) != '*' && (e & s)
									  : SymbologyKey(s) == SymbologyKey(e) && VariantKey(s) == ' ');
}

bool operator&(BarcodeFormat a, BarcodeFormat b)
{
	if (SymbologyKey(a) == '*' && SymbologyKey(b) == '*') {
		switch (VariantKey(a)) {
		case 'l': return ZXING_ENABLE_1D && !Contains("smp", VariantKey(b));
		case 's': return ZXING_ENABLE_PDF417 && !Contains("lmp", VariantKey(b));
		case 'p': return !Contains("lms", VariantKey(b)); // TODO: postal codes
		case 'm':
			return (ZXING_ENABLE_MAXICODE || ZXING_ENABLE_QRCODE || ZXING_ENABLE_DATAMATRIX || ZXING_ENABLE_AZTEC)
				   && !Contains("lsp", VariantKey(b));
		default: return true;
		}
	}

	if (SymbologyKey(a) == '*')
		std::swap(a, b);

	if (SymbologyKey(b) == '*') {
		char vkb = VariantKey(b);
		if (vkb == '*')
			return true;

		switch (a) {
#ifdef ZXING_USE_ZINT
#define USING_ZINT 1
#else
#define USING_ZINT 0
#endif
#define X(NAME, SYM, VAR, FLAGS, ZINT, ENABLED, HRI) \
	case BarcodeFormat(ZX_BCF_ID(SYM, VAR)): \
		return ENABLED \
			   && ((vkb == 'w' && USING_ZINT) ? ZINT : (FLAGS[0] == vkb || FLAGS[1] == vkb || FLAGS[2] == vkb || FLAGS[3] == vkb || FLAGS[4] == vkb));
			ZX_BCF_LIST(X)
#undef X
		};
	}

	if (a == b || Symbology(a) == b || a == Symbology(b))
		return true;

	return false;
}

// ==================================== BarcodeFormats =================================

inline bool operator<(BarcodeFormat a, BarcodeFormat b)
{
	return SymbologyKey(a) < SymbologyKey(b) || (SymbologyKey(a) == SymbologyKey(b) && VariantKey(a) < VariantKey(b));
}

void BarcodeFormats::normalize()
{
	std::erase_if(formats_, [](BarcodeFormat t) { return t == BarcodeFormat::None; });
	std::sort(formats_.begin(), formats_.end());
	formats_.erase(std::unique(formats_.begin(), formats_.end()), formats_.end());
}

BarcodeFormats::BarcodeFormats(std::string_view str)
{
	while (str.size() >= 3 && str[0] == ']') {
		formats_.push_back(BarcodeFormat(ZX_BCF_ID(str[1], str[2])));
		str.remove_prefix(3);
	}

	ForEachToken(TrimWS(str, " []"), ",|", [this](std::string_view token) {
		if (!token.empty())
			formats_.push_back(BarcodeFormatFromString(token));
	});

	normalize();
}

BarcodeFormats BarcodeFormats::list(const BarcodeFormats& filter)
{
	std::vector<BarcodeFormat> res;
	res.reserve(100);
	if (filter.empty()) {
#define X(NAME, SYM, VAR, FLAGS, ZINT, ENABLED, HRI) \
		if (ENABLED) \
			res.push_back(BarcodeFormat(ZX_BCF_ID(SYM, VAR)));
		ZX_BCF_LIST(X)
#undef X
		return res;
	}
	for (auto f : filter) {
		// printf("Filter for: %s\n", IdStr(f).c_str());
#define X(NAME, SYM, VAR, FLAGS, ZINT, ENABLED, HRI) \
	if (ENABLED && SYM != '*' \
		&& (SymbologyKey(f) == '*' ? BarcodeFormat(ZX_BCF_ID(SYM, VAR)) & f \
								   : SYM == SymbologyKey(f) && (VariantKey(f) == ' ' || VariantKey(f) == VAR))) \
		res.push_back(BarcodeFormat(ZX_BCF_ID(SYM, VAR))); //, printf("adding: %c %c\n", SYM, VAR);
		ZX_BCF_LIST(X)
#undef X
		// printf("N: %d\n", (int)res.size());
	}
	return res;
}

// BarcodeFormats&& BarcodeFormats::operator|(BarcodeFormat bt) &&
// {
// 	types_.push_back(bt);
// 	normalize();
// 	return std::move(*this);
// }

// BarcodeFormats&& BarcodeFormats::operator|(const BarcodeFormats& other) &&
// {
// 	types_.insert(std::end(types_), other.begin(), other.end());
// 	normalize();
// 	return std::move(*this);
// }

BarcodeFormats BarcodeFormats::operator&(const BarcodeFormats& other)
{
	std::vector<BarcodeFormat> res;
	std::set_intersection(begin(), end(), other.begin(), other.end(), std::back_inserter(res));
	return res;
}

BarcodeFormats BarcodeFormatsFromString(std::string_view str)
{
	return BarcodeFormats(str);
}

std::string ToString(const BarcodeFormats& formats)
{
	if (formats.empty())
		return {};
	std::string res;
	for (auto f : formats)
		res += std::string(Name(f)) + ", ";
	return res.substr(0, res.size() - 2);
}

} // namespace ZXing

#if 0
/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
*/
// SPDX-License-Identifier: Apache-2.0

#include "BarcodeFormat.h"

#include "ZXAlgorithms.h"

#include <cctype>
#include <iterator>
#include <stdexcept>

namespace ZXing {

struct BarcodeFormatName
{
	BarcodeFormat format;
	std::string_view name;
};

static const BarcodeFormatName NAMES[] = {
	{BarcodeFormat::None, "None"},
	{BarcodeFormat::Aztec, "Aztec"},
	{BarcodeFormat::Codabar, "Codabar"},
	{BarcodeFormat::Code39, "Code39"},
	{BarcodeFormat::Code93, "Code93"},
	{BarcodeFormat::Code128, "Code128"},
	{BarcodeFormat::DataBar, "DataBar"},
	{BarcodeFormat::DataBarExpanded, "DataBarExpanded"},
	{BarcodeFormat::DataBarLimited, "DataBarLimited"},
	{BarcodeFormat::DataMatrix, "DataMatrix"},
	{BarcodeFormat::DXFilmEdge, "DXFilmEdge"},
	{BarcodeFormat::EAN8, "EAN-8"},
	{BarcodeFormat::EAN13, "EAN-13"},
	{BarcodeFormat::ITF, "ITF"},
	{BarcodeFormat::MaxiCode, "MaxiCode"},
	{BarcodeFormat::MicroQRCode, "MicroQRCode"},
	{BarcodeFormat::PDF417, "PDF417"},
	{BarcodeFormat::QRCode, "QRCode"},
	{BarcodeFormat::RMQRCode, "rMQRCode"},
	{BarcodeFormat::UPCA, "UPC-A"},
	{BarcodeFormat::UPCE, "UPC-E"},
	{BarcodeFormat::LinearCodes, "Linear-Codes"},
	{BarcodeFormat::MatrixCodes, "Matrix-Codes"},
};

std::string ToString(BarcodeFormat format)
{
	auto i = FindIf(NAMES, [format](auto& v) { return v.format == format; });
	return i == std::end(NAMES) ? std::string() : std::string(i->name);
}

std::string ToString(BarcodeFormats formats)
{
	if (formats.empty())
		return ToString(BarcodeFormat::None);
	std::string res;
	for (auto f : formats)
		res += ToString(f) + "|";
	return res.substr(0, res.size() - 1);
}

BarcodeFormat BarcodeFormatFromString(std::string_view str)
{
	auto i = FindIf(NAMES, [str](auto& v) { return IsEqualIgnoreCaseAnd(v.name, str, "_-"); });
	return i == std::end(NAMES) ? BarcodeFormat::None : i->format;
}

BarcodeFormats BarcodeFormatsFromString(std::string_view str)
{
	BarcodeFormats res;
	ForEachToken(TrimWS(str, " []"), " ,|", [&res](std::string_view token) {
		if (!token.empty()) {
			auto bc = BarcodeFormatFromString(token);
			if (bc == BarcodeFormat::None)
				throw std::invalid_argument("This is not a valid barcode format: '" + std::string(token) + "'");
			res |= bc;
		}
	});
	return res;
}

} // ZXing
#endif // 0
