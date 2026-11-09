/*
* Copyright 2024 Axel Waggershauser
* Copyright 2025 gitlost
*/
// SPDX-License-Identifier: Apache-2.0

#include "CreateBarcode.h"

#include "BarcodeData.h"
#include "BitMatrix.h"
#include "ByteArray.h"
#include "DecoderResult.h"
#include "DetectorResult.h"
#include "JSON.h"

#if !defined(ZXING_READERS) && !defined(ZXING_WRITERS)
#include "Version.h"
#endif

#ifdef ZXING_READERS
#include "ReadBarcode.h"
#endif

#ifdef ZXING_USE_ZINT
#include "ECI.h"
#include "TextEncoder.h"
#include <zint.h>
#else
#include "MultiFormatWriter.h"
#endif // ZXING_USE_ZINT

#include <charconv>
#include <optional>

using namespace std::literals;

namespace ZXing {

struct CreatorOptions::Data
{
	BarcodeFormat format;
	std::string options;

	// symbol size (qrcode, datamatrix, etc), map from I, 'WxH'
	// structured_append (idx, cnt, ID)

	mutable unique_zint_symbol zint;
};

// TODO: check return type
#define ZX_PROPERTY(TYPE, NAME) \
	const TYPE& CreatorOptions::NAME() const noexcept { return d->NAME; } \
	CreatorOptions& CreatorOptions::NAME(TYPE v)& { return d->NAME = std::move(v), *this; } \
	CreatorOptions&& CreatorOptions::NAME(TYPE v)&& { return d->NAME = std::move(v), std::move(*this); }

ZX_PROPERTY(BarcodeFormat, format)
ZX_PROPERTY(std::string, options)

#undef ZX_PROPERTY

#define ZX_RO_PROPERTY(TYPE, NAME) \
	std::optional<TYPE> CreatorOptions::NAME() const noexcept { return JsonGet<TYPE>(d->options, #NAME); }

ZX_RO_PROPERTY(std::string, ecLevel);
ZX_RO_PROPERTY(std::string, eci);
ZX_RO_PROPERTY(bool, gs1);
ZX_RO_PROPERTY(bool, readerInit);
ZX_RO_PROPERTY(bool, forceSquare);
ZX_RO_PROPERTY(int, columns);
ZX_RO_PROPERTY(int, rows);
ZX_RO_PROPERTY(int, version);
ZX_RO_PROPERTY(int, dataMask);

#undef ZX_RO_PROPERTY

CreatorOptions::CreatorOptions(BarcodeFormat format, std::string options) : d(std::make_unique<Data>(format, std::move(options))) {}
CreatorOptions::~CreatorOptions() = default;
CreatorOptions::CreatorOptions(CreatorOptions&&) noexcept = default;
CreatorOptions& CreatorOptions::operator=(CreatorOptions&&) noexcept = default;

inline bool IsAscii(ByteView bv)
{
	return std::all_of(bv.begin(), bv.end(), [](uint8_t c) { return c <= 127; });
}

#ifdef ZXING_WRITERS
#ifdef ZXING_USE_ZINT

struct String2Int
{
	const char* str;
	int val;
};

static int ParseECLevel(int symbology, std::string_view s)
{
	constexpr std::string_view EC_LABELS_QR[4] = {"L", "M", "Q", "H"};
	int res = 0;

	// Convert L/M/Q/H to Zint 1-4
	if (Contains({BARCODE_QRCODE, BARCODE_MICROQR, BARCODE_RMQR}, symbology))
		if ((res = IndexOf(EC_LABELS_QR, s)) != -1)
			return res + 1;

	if (std::from_chars(s.data(), s.data() + s.size() - (s.back() == '%'), res).ec != std::errc{})
		throw std::invalid_argument(StrCat("Invalid ecLevel: '", s, "'"));

	auto findClosestECLevel = [](const std::vector<int>& list, int val) {
		int mIdx = -2, mAbs = 100;
		for (int i = 0; i < Size(list); ++i)
			if (int abs = std::abs(val - list[i]); abs < mAbs) {
				mIdx = i;
				mAbs = abs;
			}
		return mIdx + 1;
	};

	// Convert percentage to Zint
	if (s.back() == '%') {
		switch (symbology) {
		case BARCODE_QRCODE: return findClosestECLevel({20, 37, 55, 65}, res);
		case BARCODE_MICROQR: return findClosestECLevel({20, 37, 55}, res);
		case BARCODE_RMQR: return res <= 46 ? 2 : 4;
		case BARCODE_AZTEC: return findClosestECLevel({10, 23, 36, 50}, res);
		case BARCODE_PDF417:
			// TODO: do something sensible with PDF417?
		default:
			return -1;
		}
	}

	return res;
};

static constexpr struct { BarcodeFormat format; SymbologyIdentifier si; } barcodeFormat2SymbologyIdentifier[] = {
	{BarcodeFormat::Aztec, {'z', '0', 3}}, // '1' GS1, '2' AIM
	{BarcodeFormat::Codabar, {'F', '0'}}, // if checksum processing were implemented and checksum present and stripped then modifier would be 4
	// {BarcodeFormat::CodablockF, {'O', '4'}}, // '5' GS1
	{BarcodeFormat::Code128, {'C', '0'}}, // '1' GS1, '2' AIM
	// {BarcodeFormat::Code16K, {'K', '0'}}, // '1' GS1, '2' AIM, '4' D1 PAD
	// {BarcodeFormat::Code39, {'A', '0'}}, // '3' checksum, '4' extended, '7' checksum,extended
	{BarcodeFormat::Code39Ext, {'A', '4'}}, // '3' checksum, '4' extended, '7' checksum,extended
	{BarcodeFormat::DataBar, {'e', '0', 0, AIFlag::GS1}},
	{BarcodeFormat::DataBarOmni, {'e', '0', 0, AIFlag::GS1}},
	{BarcodeFormat::DataBarStk, {'e', '0', 0, AIFlag::GS1}},
	{BarcodeFormat::DataBarStkOmni, {'e', '0', 0, AIFlag::GS1}},
	{BarcodeFormat::DataBarExp, {'e', '0', 0, AIFlag::GS1}},
	{BarcodeFormat::DataBarExpStk, {'e', '0', 0, AIFlag::GS1}},
	{BarcodeFormat::DataBarLtd, {'e', '0', 0, AIFlag::GS1}},
	{BarcodeFormat::DataMatrix, {'d', '1', 3}}, // '2' GS1, '3' AIM
	// {BarcodeFormat::DotCode, {'J', '0', 3}}, // '1' GS1, '2' AIM
	{BarcodeFormat::DXFilmEdge, {'X', 'F'}},
	{BarcodeFormat::EAN8, {'E', '4'}},
	// {BarcodeFormat::HanXin, {'h', '0', 1}}, // '2' GS1
	// {BarcodeFormat::ITF, {'I', '0'}}, // '1' check digit
	{BarcodeFormat::MaxiCode, {'U', '0', 2}}, // '1' mode 2 or 3
	{BarcodeFormat::MicroPDF417, {'L', '2', -1}},
	{BarcodeFormat::MicroQRCode, {'Q', '1', 1}},
	{BarcodeFormat::PDF417, {'L', '2', -1}},
	{BarcodeFormat::QRCode, {'Q', '1', 1}}, // '3' GS1, '5' AIM
	{BarcodeFormat::RMQRCode, {'Q', '1', 1}}, // '3' GS1, '5' AIM
};

static SymbologyIdentifier SymbologyIdentifierZint2ZXing(const CreatorOptions& opts, const ByteArray& ba)
{
	using enum BarcodeFormat;

	const BarcodeFormat format = opts.format();

	auto i = FindIf(barcodeFormat2SymbologyIdentifier, [format](auto& v) { return v.format == format; });
	SymbologyIdentifier ret = i != std::end(barcodeFormat2SymbologyIdentifier) ? i->si : SymbologyIdentifier{SymbologyKey(format), '0'};

	if (format & (EAN13 | UPCA | UPCE)) {
		if (ba.size() > 13)     // Have EAN-2/5 add-on?
			ret.modifier = '3'; // Combined packet, EAN-13, UPC-A, UPC-E, with add-on
	} else if (opts.gs1() && format & AllGS1) {
		if (format & (Aztec | Code128))
			ret.modifier = '1';
		else if (format == DataMatrix)
			ret.modifier = '2';
		else if (format & (QRCode | RMQRCode))
			ret.modifier = '3';
		ret.aiFlag = AIFlag::GS1;
	}

	return ret;
}

static std::string ECLevelZint2ZXing(const zint_symbol* zint)
{
	constexpr char EC_LABELS_QR[4] = {'L', 'M', 'Q', 'H'};

	const int symbology = zint->symbology;
	const int option_1 = zint->option_1;

	switch (symbology) {
	case BARCODE_AZTEC:
		if ((option_1 >> 8) >= 0 && (option_1 >> 8) <= 99)
			return std::to_string(option_1 >> 8) + "%";
		break;
	case BARCODE_MAXICODE:
		// Mode
		if (option_1 >= 2 && option_1 <= 6)
			return std::to_string(option_1);
		break;
	case BARCODE_PDF417:
	case BARCODE_PDF417COMP:
		// Convert to percentage
		if (option_1 >= 0 && option_1 <= 8) {
			int overhead = symbology == BARCODE_PDF417COMP ? 35 : 69;
			int cols = (zint->width - overhead) / 17;
			int tot_cws = zint->rows * cols;
			assert(tot_cws);
			return std::to_string((2 << option_1) * 100 / tot_cws) + "%";
		}
		break;
	// case BARCODE_MICROPDF417:
	// 	if ((option_1 >> 8) >= 0 && (option_1 >> 8) <= 99)
	// 		return std::to_string(option_1 >> 8) + "%";
	// 	break;
	case BARCODE_QRCODE:
	case BARCODE_MICROQR:
	case BARCODE_RMQR:
		// Convert to L/M/Q/H
		if (option_1 >= 1 && option_1 <= 4)
			return {EC_LABELS_QR[option_1 - 1]};
		break;
	// case BARCODE_HANXIN:
	// 	if (option_1 >= 1 && option_1 <= 4)
	// 		return "L" + std::to_string(option_1);
	// 	break;
	default:
		break;
	}

	return {};
}

#ifndef ZXING_READERS
// Convert bytes to UTF-8 code points 0-255
static std::string BinaryToUtf8(ByteView ba)
{
	std::string utf8;
	utf8.reserve(ba.size() * 2);
	for (auto c : ba)
		if (c < 0x80) {
			utf8.push_back(c);
		} else if (c < 0xC0) {
			utf8.push_back(0xC2);
			utf8.push_back(c);
		} else {
			utf8.push_back(0xC3);
			utf8.push_back(c - 0x40);
		}
	return utf8;
}
#endif

zint_symbol* CreatorOptions::zint() const
{
	using enum BarcodeFormat;

	auto& zint = d->zint;

	if (!zint) {
#ifdef PRINT_DEBUG
//		printf("zint version: %d, sizeof(zint_symbol): %ld, options: %s\n", ZBarcode_Version(), sizeof(zint_symbol), options().c_str());
#endif
		zint.reset(ZBarcode_Create());

		switch (format()) {
#define X(NAME, SYM, VAR, FLAGS, ZINT, ENABLED, HRI) \
	case BarcodeFormat(ZX_BCF_ID(SYM, VAR)): zint->symbology = ZINT; break;
			ZX_BCF_LIST(X)
#undef X
		};

		if (zint->symbology == 0)
			throw std::invalid_argument(StrCat("Unsupported barcode format for creation: ", ToString(format())));

		if (format() == Code128 && gs1())
			zint->symbology = BARCODE_GS1_128;

		zint->scale = 0.5f;

		if (auto val = ecLevel(); val)
			zint->option_1 = ParseECLevel(zint->symbology, *val);

		if (auto val = version(); val && !(format() & AllLinear))
			zint->option_2 = *val;

		if (auto val = columns(); val && format() & (DataBarExpStk | PDF417 | MicroPDF417 | CompactPDF417))
			zint->option_2 = *val;

		if (auto val = rows(); val && format() & (DataBarExpStk | PDF417))
			zint->option_3 = *val;

		if (auto val = dataMask(); val && format() & (QRCode | MicroQRCode))
			zint->option_3 = (zint->option_3 & 0xFF) | (*val + 1) << 8;

		if (format() == DataMatrix)
			zint->option_3 = (forceSquare() ? DM_SQUARE : DM_DMRE) | DM_ISO_144;
	}

	return zint.get();
}

#define CHECK_WARN(ZINT_CALL, WARN) \
	if (WARN = (ZINT_CALL); WARN >= ZINT_ERROR) \
		throw std::invalid_argument(StrCat(zint->errtxt, " (retval: ", std::to_string(WARN), ")"));

Barcode CreateBarcode(const void* data, int size, int mode, const CreatorOptions& opts)
{
	auto zint = opts.zint();

	zint->input_mode = mode == UNICODE_MODE && opts.gs1() && (opts.format() & BarcodeFormat::AllGS1) ? GS1_MODE : mode;
	if (mode == UNICODE_MODE && static_cast<const char*>(data)[0] != '[')
		zint->input_mode |= GS1PARENS_MODE;
	zint->output_options |= OUT_BUFFER_INTERMEDIATE | BARCODE_NO_QUIET_ZONES | BARCODE_CONTENT_SEGS;
	if (opts.readerInit())
		zint->output_options |= READER_INIT;

	if (ZBarcode_Cap(zint->symbology, ZINT_CAP_ECI)) {
		if (auto eci = opts.eci(); eci) {
			if (auto cs = CharacterSetFromString(*eci); cs != CharacterSet::Unknown) {
				zint->eci = static_cast<int>(ToECI(cs));
			} else if (std::all_of(eci->begin(), eci->end(), [](char c) { return std::isdigit(c); })) {
				zint->eci = std::stoi(*eci);
			}
		} else if (mode == DATA_MODE) {
			zint->eci = static_cast<int>(ECI::Binary);
		}
		// the following would make sure that non-ASCII text is encoded as UTF-8, hence the GuessTextEncoding for
		// non-ECI data can't produce a wrong result.
		// else if (mode == UNICODE_MODE && !IsAscii({data, narrow_cast<size_t>(size)}))
		// 	zint->eci = static_cast<int>(ECI::UTF8);
	}

	int warning;
	CHECK_WARN(ZBarcode_Encode_and_Buffer(zint, (uint8_t*)data, size, 0), warning);

#ifdef PRINT_DEBUG
	printf("create symbol with size: %dx%d\n", zint->width, zint->rows);
#endif

#if 0 // use ReadBarcode to create Barcode object
	auto buffer = std::vector<uint8_t>(zint->bitmap_width * zint->bitmap_height);
	std::transform(zint->bitmap, zint->bitmap + zint->bitmap_width * zint->bitmap_height, buffer.data(),
				   [](unsigned char v) { return (v == '0') * 0xff; });

	auto res = ReadBarcode({buffer.data(), zint->bitmap_width, zint->bitmap_height, ImageFormat::Lum},
						   ReaderOptions().formats(opts.format()).isPure(true).binarizer(Binarizer::BoolCast));
#else
	assert(zint->content_seg_count == 1);
	const auto& content_seg = zint->content_segs[0];
	const size_t content_seg_len =
		static_cast<size_t>(content_seg.length - (opts.format() == BarcodeFormat::Code93 && content_seg.length >= 2 ? 2 : 0));

	Content content;

	if (zint->eci || warning == ZINT_WARN_USES_ECI)
		content.switchEncoding(ECI(content_seg.eci));
	else
		content.switchEncoding(ToCharacterSet(ECI(content_seg.eci)));

	if ((zint->input_mode & 0x07) == UNICODE_MODE) {
		// `content_segs` returned as UTF-8
		std::string utf8(reinterpret_cast<const char *>(content_seg.source), content_seg_len);
		content.append(TextEncoder::FromUnicode(utf8, ToCharacterSet(ECI(content_seg.eci))));
#ifndef ZXING_READERS
		content.utf8Cache.push_back(std::move(utf8));
#endif
	} else {
		content.append({content_seg.source, content_seg_len});
#ifndef ZXING_READERS
		content.utf8Cache.push_back(BinaryToUtf8(content.bytes));
#endif
	}

	content.symbology = SymbologyIdentifierZint2ZXing(opts, content.bytes);

	DecoderResult decRes(std::move(content));
	decRes.setEcLevel(ECLevelZint2ZXing(zint));
	decRes.setReaderInit(zint->output_options & READER_INIT);
	auto bits = BitMatrix(zint->bitmap_width, zint->bitmap_height);
	std::transform(zint->bitmap, zint->bitmap + zint->bitmap_width * zint->bitmap_height, bits.row(0).begin(),
				   [](unsigned char v) { return (v == '1') * BitMatrix::SET_V; });
	int left, top, width, height;
	bits.findBoundingBox(left, top, width, height);

	auto res = MatrixBarcode(std::move(decRes), {std::move(bits), Rectangle<PointI>(left, top, width, height)}, opts.format());
#endif

	res.zint = std::move(opts.d->zint);

	return res;
}

Barcode CreateBarcodeFromText(std::string_view contents, const CreatorOptions& options)
{
	return CreateBarcode(contents.data(), contents.size(), UNICODE_MODE, options);
}

Barcode CreateBarcodeFromText(std::u8string_view contents, const CreatorOptions& options)
{
	return CreateBarcode(contents.data(), contents.size(), UNICODE_MODE, options);
}

Barcode CreateBarcodeFromBytes(const void* data, int size, const CreatorOptions& options)
{
	return CreateBarcode(data, size, DATA_MODE, options);
}

#else // ZXING_USE_ZINT

zint_symbol* CreatorOptions::zint() const { return nullptr; }

static Barcode CreateBarcode(BitMatrix&& bits, std::string_view contents, const CreatorOptions& opts)
{
	Content content;
	content.append(contents);

	DecoderResult decRes(std::move(content));
	DetectorResult detRes(std::move(bits), Rectangle<PointI>(0, 0, bits.width(), bits.height()));
	return MatrixBarcode(std::move(decRes), std::move(detRes), opts.format());
}

Barcode CreateBarcodeFromText(std::string_view contents, const CreatorOptions& opts)
{
	auto writer = MultiFormatWriter(opts.format()).setMargin(0);
	if (auto ecLevel = opts.ecLevel(); ecLevel && ecLevel->size() == 1 && strchr("012345678", (*ecLevel)[0]))
		writer.setEccLevel(std::stoi(*ecLevel));
	if (!IsAscii({(const uint8_t*)contents.data(), contents.size()}))
		writer.setEncoding(CharacterSet::UTF8); // write UTF8 (ECI value 26) for maximum compatibility

	return CreateBarcode(writer.encode(std::string(contents), 0, opts.format() & BarcodeFormat::AllLinear ? 50 : 0), contents, opts);
}

Barcode CreateBarcodeFromText(std::u8string_view contents, const CreatorOptions& opts)
{
	return CreateBarcodeFromText({reinterpret_cast<const char*>(contents.data()), contents.size()}, opts);
}

Barcode CreateBarcodeFromBytes(const void* data, int size, const CreatorOptions& opts)
{
	std::wstring bytes;
	for (uint8_t c : ByteView(data, size))
		bytes.push_back(c);

	auto writer = MultiFormatWriter(opts.format()).setMargin(0);
	if (auto ecLevel = opts.ecLevel(); ecLevel && ecLevel->size() == 1 && strchr("012345678", (*ecLevel)[0]))
		writer.setEccLevel(std::stoi(*ecLevel));
	writer.setEncoding(CharacterSet::BINARY);

	return CreateBarcode(writer.encode(bytes, 0, opts.format() & BarcodeFormat::AllLinear ? 50 : 0), {(const char*)data, (size_t)size}, opts);
}

#endif // ZXING_USE_ZINT

#else // ZXING_WRITERS

zint_symbol* CreatorOptions::zint() const { return nullptr; }

Barcode CreateBarcodeFromText(std::string_view, const CreatorOptions&)
{
	throw std::runtime_error("This build of zxing-cpp does not support creating barcodes.");
}

Barcode CreateBarcodeFromText(std::u8string_view, const CreatorOptions&)
{
	throw std::runtime_error("This build of zxing-cpp does not support creating barcodes.");
}

Barcode CreateBarcodeFromBytes(const void*, int, const CreatorOptions&)
{
	throw std::runtime_error("This build of zxing-cpp does not support creating barcodes.");
}

#endif // ZXING_WRITERS

} // namespace ZXing
