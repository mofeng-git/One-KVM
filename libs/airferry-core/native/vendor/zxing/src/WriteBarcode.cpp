/*
* Copyright 2024 Axel Waggershauser
* Copyright 2025 gitlost
*/
// SPDX-License-Identifier: Apache-2.0

#include "WriteBarcode.h"

#include "BarcodeData.h"
#include "BitMatrix.h"
#include "CreateBarcode.h"

#if !defined(ZXING_READERS) && !defined(ZXING_WRITERS)
#include "Version.h"
#endif

#include <sstream>

#ifdef ZXING_USE_ZINT
#include <zint.h>
#endif // ZXING_USE_ZINT

namespace ZXing {

struct WriterOptions::Data
{
	int scale = 1;
	int rotate = 0;
	bool invert = false;
	bool addHRT = false;
	bool addQuietZones = true;
};

#define ZX_PROPERTY(TYPE, NAME) \
	TYPE WriterOptions::NAME() const noexcept { return d->NAME; } \
	WriterOptions& WriterOptions::NAME(TYPE v)& { return d->NAME = std::move(v), *this; } \
	WriterOptions&& WriterOptions::NAME(TYPE v)&& { return d->NAME = std::move(v), std::move(*this); }

ZX_PROPERTY(int, scale)
ZX_PROPERTY(int, rotate)
ZX_PROPERTY(bool, invert)
ZX_PROPERTY(bool, addHRT)
ZX_PROPERTY(bool, addQuietZones)

#undef ZX_PROPERTY

WriterOptions::WriterOptions() : d(std::make_unique<Data>()) {}
WriterOptions::~WriterOptions() = default;
WriterOptions::WriterOptions(WriterOptions&&) noexcept = default;
WriterOptions& WriterOptions::operator=(WriterOptions&&) noexcept = default;

#ifdef ZXING_USE_ZINT

struct SetCommonWriterOptions
{
	zint_symbol* zint;

	SetCommonWriterOptions(zint_symbol* zint, const WriterOptions& opts) : zint(zint)
	{
		zint->show_hrt = opts.addHRT();

		zint->output_options &= ~(OUT_BUFFER_INTERMEDIATE | BARCODE_NO_QUIET_ZONES);
		zint->output_options |= opts.addQuietZones() ? BARCODE_QUIET_ZONES : BARCODE_NO_QUIET_ZONES;

		if (opts.scale() > 0)
			zint->scale = opts.scale() / 2.f;
		else if (opts.scale() < 0) {
			int size = std::max(zint->width, zint->rows);
			zint->scale = std::max(1, int(float(-opts.scale()) / size)) / 2.f;
		}

		if (opts.invert()) {
			strcpy(zint->bgcolour, "000000");
			strcpy(zint->fgcolour, "ffffff");
		}
	}

	// reset the defaults such that consecutive write calls don't influence each other
	~SetCommonWriterOptions()
	{
		zint->scale = 0.5f;
		strcpy(zint->fgcolour, "000000");
		strcpy(zint->bgcolour, "ffffff");
	}
};

#define CHECK(ZINT_CALL) \
	if (int err = (ZINT_CALL); err >= ZINT_ERROR) \
		throw std::invalid_argument(StrCat(zint->errtxt, " (retval: ", std::to_string(err), ")"));

#endif // ZXING_USE_ZINT

static std::string ToSVG(ImageView iv)
{
	if (!iv.data())
		return {};

	// see https://stackoverflow.com/questions/10789059/create-qr-code-in-vector-image/60638350#60638350

	std::ostringstream res;

	res << R"(<?xml version="1.0" encoding="UTF-8"?>)" << "\n"
		<< R"(<svg xmlns="http://www.w3.org/2000/svg" version="1.1" viewBox="0 0 )" << iv.width() << ' ' << iv.height()
		<< R"(" stroke="none">)" << "\n"
		<< "<path d=\"";

	for (int y = 0; y < iv.height(); ++y)
		for (int x = 0; x < iv.width(); ++x)
			if (*iv.data(x, y) == 0)
				res << "M" << x << "," << y << "h1v1h-1z";

	res << "\"/>\n</svg>";

	return res.str();
}

static Image ToImage(BitMatrix bits, bool isLinearCode, const WriterOptions& opts)
{
	bits.flipAll();
	int width = opts.scale() > 0 ? bits.width() * opts.scale() : -opts.scale();
	int height = isLinearCode ? std::clamp(width / 2, 50, 300) : opts.scale() > 0 ? bits.height() * opts.scale() : -opts.scale();
	auto symbol = Inflate(std::move(bits), width, height, opts.addQuietZones() ? 10 : 0);
	auto bitmap = ToMatrix<uint8_t>(symbol);
	auto iv = Image(symbol.width(), symbol.height());
	std::memcpy(const_cast<uint8_t*>(iv.data()), bitmap.data(), iv.width() * iv.height());
	return iv;
}

std::string WriteBarcodeToSVG(const Barcode& barcode, [[maybe_unused]] const WriterOptions& options)
{
#if defined(ZXING_WRITERS) && defined(ZXING_USE_ZINT)
	auto* zint = barcode.d->zint.get();

	if (!zint)
#endif
		return ToSVG(barcode.symbol());

#if defined(ZXING_WRITERS) && defined(ZXING_USE_ZINT)
	auto resetOnExit = SetCommonWriterOptions(zint, options);

	zint->output_options |= BARCODE_MEMORY_FILE;// | EMBED_VECTOR_FONT;
	strcpy(zint->outfile, "null.svg");

	CHECK(ZBarcode_Print(zint, options.rotate()));

	return std::string(reinterpret_cast<const char*>(zint->memfile), zint->memfile_size);
#endif
}

Image WriteBarcodeToImage(const Barcode& barcode, [[maybe_unused]] const WriterOptions& options)
{
#if defined(ZXING_WRITERS) && defined(ZXING_USE_ZINT)
	auto* zint = barcode.d->zint.get();

	if (!zint)
#endif
		return ToImage(barcode.d->symbol.copy(), barcode.format() & BarcodeFormat::AllLinear, options);

#if defined(ZXING_WRITERS) && defined(ZXING_USE_ZINT)
	auto resetOnExit = SetCommonWriterOptions(zint, options);

	CHECK(ZBarcode_Buffer(zint, options.rotate()));

#ifdef PRINT_DEBUG
	printf("write symbol with size: %dx%d\n", zint->bitmap_width, zint->bitmap_height);
#endif
	auto iv = Image(zint->bitmap_width, zint->bitmap_height);
	auto* src = zint->bitmap;
	auto* dst = const_cast<uint8_t*>(iv.data());
	for(int y = 0; y < iv.height(); ++y)
		for(int x = 0, w = iv.width(); x < w; ++x, src += 3)
			*dst++ = RGBToLum(src[0], src[1], src[2]);

	return iv;
#endif
}

std::string WriteBarcodeToUtf8(const Barcode& barcode, [[maybe_unused]] const WriterOptions& options)
{
	auto iv = barcode.symbol();
	if (!iv.data())
		return {};

	constexpr auto map = std::array{" ", "▀", "▄", "█"};
	std::ostringstream res;
	bool invert = !options.invert();

	Image buffer;
	if (options.addQuietZones()) {
		buffer = Image(iv.width() + 2, iv.height() + 2);
		memset(const_cast<uint8_t*>(buffer.data()), 0xff, buffer.rowStride() * buffer.height());
		for (int y = 0; y < iv.height(); y++)
			memcpy(const_cast<uint8_t*>(buffer.data(1, y + 1)), iv.data(0, y), iv.width());
		iv = barcode.format() & BarcodeFormat::AllLinear ? iv.cropped(0, 1, buffer.width(), buffer.height() - 2) : buffer;
	}

	for (int y = 0; y < iv.height(); y += 2) {
		// for linear barcodes, only print line pairs that are distinct from the previous one
		if (barcode.format() & BarcodeFormat::AllLinear && y > 1 && y < iv.height() - 1
			&& memcmp(iv.data(0, y), iv.data(0, y - 2), 2 * iv.rowStride()) == 0)
			continue;

		for (int x = 0; x < iv.width(); ++x) {
			int tp = bool(*iv.data(x, y)) ^ invert;
			int bt = (iv.height() == 1 && tp) || (y + 1 < iv.height() && (bool(*iv.data(x, y + 1)) ^ invert));
			res << map[tp | (bt << 1)];
		}
		res << '\n';
	}

	return res.str();
}

} // namespace ZXing
