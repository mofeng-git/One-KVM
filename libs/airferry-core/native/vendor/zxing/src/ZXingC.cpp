/*
* Copyright 2023 siiky
* Copyright 2023 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#include "ZXingC.h"

#include "ZXingCpp.h"
#include "ZXAlgorithms.h"
#include "ZXConfig.h"
#include "Version.h"

#include <bit>
#include <cstdlib>
#include <exception>
#include <string>
#include <string_view>
#include <utility>

using namespace ZXing;

static ZX_THREAD_LOCAL std::string lastErrorMsg;
static Barcodes emptyBarcodes{}; // used to prevent new heap allocation for each empty result

template<typename R, typename T> R transmute_cast(const T& v) noexcept
{
	static_assert(sizeof(T) == sizeof(R));
	return *(const R*)(&v);
}

template<typename C, typename P = typename C::pointer>
P copy(const C& c) noexcept
{
	auto ret = (P)malloc((c.size() + 1) * sizeof(typename C::value_type));
	if (ret) {
		memcpy(ret, c.data(), c.size() * sizeof(typename C::value_type));
		ret[c.size()] = typename C::value_type(0);
	}
	return ret;
}

template<typename C, typename P = typename C::pointer>
P copy(const C& c, int* len) noexcept
{
	// for convenience and as a safety measure, we NULL terminate even byte arrays
	auto ret = copy(c);
	if (len)
		*len = ret ? Size(c) : 0;
	return ret;
}

#define ZX_CHECK( GOOD, MSG ) \
	if (!(GOOD)) { \
		lastErrorMsg = MSG; \
		return {}; \
	}

#define ZX_CATCH(...) \
	catch (std::exception & e) { \
		lastErrorMsg = e.what(); \
	} catch (...) { \
		lastErrorMsg = "Unknown error"; \
	} \
	return __VA_ARGS__;

#define ZX_TRY(...) \
	try { \
		return __VA_ARGS__; \
	} \
	ZX_CATCH({})


extern "C" {

/*
 * MARK: - ImageView.h
 */

ZXing_ImageView* ZXing_ImageView_new(const uint8_t* data, int width, int height, ZXing_ImageFormat format, int rowStride,
									 int pixStride)
{
	auto cppformat = static_cast<ImageFormat>(format);
	ZX_TRY(new ImageView(data, width, height, cppformat, rowStride, pixStride))
}

ZXing_ImageView* ZXing_ImageView_new_checked(const uint8_t* data, int size, int width, int height, ZXing_ImageFormat format,
											 int rowStride, int pixStride)
{
	auto cppformat = static_cast<ImageFormat>(format);
	ZX_TRY(new ImageView(data, size, width, height, cppformat, rowStride, pixStride))
}

void ZXing_ImageView_delete(ZXing_ImageView* iv)
{
	delete iv;
}

void ZXing_ImageView_crop(ZXing_ImageView* iv, int left, int top, int width, int height)
{
	*iv = iv->cropped(left, top, width, height);
}

void ZXing_ImageView_rotate(ZXing_ImageView* iv, int degree)
{
	*iv = iv->rotated(degree);
}

void ZXing_Image_delete(ZXing_Image* img)
{
	delete img;
}

const uint8_t* ZXing_Image_data(const ZXing_Image* img)
{
	return img->data();
}

int ZXing_Image_width(const ZXing_Image* img)
{
	return img->width();
}

int ZXing_Image_height(const ZXing_Image* img)
{
	return img->height();
}

ZXing_ImageFormat ZXing_Image_format(const ZXing_Image* img)
{
	return transmute_cast<ZXing_ImageFormat>(img->format());
}

/*
 * MARK: - BarcodeFormat.h
 */

ZXing_BarcodeFormat ZXing_BarcodeFormatSymbology(ZXing_BarcodeFormat format)
{
	return transmute_cast<ZXing_BarcodeFormat>(Symbology(transmute_cast<BarcodeFormat>(format)));
}

ZXing_BarcodeFormat ZXing_BarcodeFormatFromString(const char* str)
{
	try {
		return transmute_cast<ZXing_BarcodeFormat>(BarcodeFormatFromString(str));
	}
	ZX_CATCH(ZXing_BarcodeFormat_Invalid)
}

char* ZXing_BarcodeFormatToString(ZXing_BarcodeFormat format)
{
	try {
		return copy(ToString(transmute_cast<BarcodeFormat>(format)));
	}
	ZX_CATCH(NULL)
}

ZXing_BarcodeFormat* ZXing_BarcodeFormatsList(ZXing_BarcodeFormat filter, int* outCount)
{
	try {
		return (ZXing_BarcodeFormat*)copy(BarcodeFormats::list(transmute_cast<BarcodeFormat>(filter)), outCount);
	}
	ZX_CATCH(NULL)
}

ZXing_BarcodeFormat* ZXing_BarcodeFormatsFromString(const char* str, int* outCount)
{
	if (!str) {
		if (outCount)
			*outCount = 0;
		return NULL;
	}
	try {
		return (ZXing_BarcodeFormat*)copy(BarcodeFormats(str), outCount);
	}
	ZX_CATCH(NULL)
}

char* ZXing_BarcodeFormatsToString(const ZXing_BarcodeFormat* formats, int count)
{
	if (!formats || count == 0)
		return copy(std::string{});
	try {
		std::vector<BarcodeFormat> v((BarcodeFormat*)formats, (BarcodeFormat*)formats + count);
		return copy(ToString(BarcodeFormats(std::move(v))));
	}
	ZX_CATCH(NULL)
}

/*
 * MARK: - Barcode.h
 */

char* ZXing_ContentTypeToString(ZXing_ContentType type)
{
	return copy(ToString(static_cast<ContentType>(type)));
}

char* ZXing_PositionToString(ZXing_Position position)
{
	return copy(ToString(transmute_cast<Position>(position)));
}


bool ZXing_Barcode_isValid(const ZXing_Barcode* barcode)
{
	return barcode != NULL && barcode->isValid();
}

ZXing_ErrorType ZXing_Barcode_errorType(const ZXing_Barcode* barcode)
{
	return static_cast<ZXing_ErrorType>(barcode->error().type());
}

char* ZXing_Barcode_errorMsg(const ZXing_Barcode* barcode)
{
	return copy(ToString(barcode->error()));
}

uint8_t* ZXing_Barcode_bytes(const ZXing_Barcode* barcode, int* len)
{
	return copy(barcode->bytes(), len);
}

uint8_t* ZXing_Barcode_bytesECI(const ZXing_Barcode* barcode, int* len)
{
	return copy(barcode->bytesECI(), len);
}

char* ZXing_Barcode_extra(const ZXing_Barcode* barcode, const char* key)
{
	return copy(barcode->extra(key ? key : ""));
}

#define ZX_GETTER(TYPE, NAME, TRANS) \
	TYPE ZXing_Barcode_##NAME(const ZXing_Barcode* barcode) { return TRANS(barcode->NAME()); }

ZX_GETTER(ZXing_BarcodeFormat, format, static_cast<ZXing_BarcodeFormat>)
ZX_GETTER(ZXing_BarcodeFormat, symbology, static_cast<ZXing_BarcodeFormat>)
ZX_GETTER(ZXing_ContentType, contentType, static_cast<ZXing_ContentType>)
ZX_GETTER(char*, text, copy)
ZX_GETTER(char*, ecLevel, copy)
ZX_GETTER(char*, symbologyIdentifier, copy)
ZX_GETTER(char*, sequenceId, copy)
ZX_GETTER(ZXing_Position, position, transmute_cast<ZXing_Position>)

ZX_GETTER(int, orientation,)
ZX_GETTER(bool, hasECI,)
ZX_GETTER(bool, isInverted,)
ZX_GETTER(bool, isMirrored,)
ZX_GETTER(int, lineCount,)
ZX_GETTER(int, sequenceIndex,)
ZX_GETTER(int, sequenceSize,)

void ZXing_Barcode_delete(ZXing_Barcode* barcode)
{
	delete barcode;
}

void ZXing_Barcodes_delete(ZXing_Barcodes* barcodes)
{
	if (barcodes != &emptyBarcodes)
		delete barcodes;
}

int ZXing_Barcodes_size(const ZXing_Barcodes* barcodes)
{
	return barcodes ? Size(*barcodes) : 0;
}

const ZXing_Barcode* ZXing_Barcodes_at(const ZXing_Barcodes* barcodes, int i)
{
	if (!barcodes || i < 0 || i >= Size(*barcodes))
		return NULL;
	return &(*barcodes)[i];
}

ZXing_Barcode* ZXing_Barcodes_move(ZXing_Barcodes* barcodes, int i)
{
	if (!barcodes || i < 0 || i >= Size(*barcodes))
		return NULL;

	ZX_TRY(new Barcode(std::move((*barcodes)[i])));
}

/*
 * MARK: - ReaderOptions.h
 */

ZXing_ReaderOptions* ZXing_ReaderOptions_new()
{
	ZX_TRY(new ReaderOptions());
}

void ZXing_ReaderOptions_delete(ZXing_ReaderOptions* opts)
{
	delete opts;
}

#define ZX_PROPERTY(TYPE, NAME, CAP_NAME) \
	TYPE ZXing_ReaderOptions_get##CAP_NAME(const ZXing_ReaderOptions* opts) { return opts->NAME(); } \
	void ZXing_ReaderOptions_set##CAP_NAME(ZXing_ReaderOptions* opts, TYPE NAME) { opts->NAME(NAME); }

ZX_PROPERTY(bool, tryHarder, TryHarder)
ZX_PROPERTY(bool, tryRotate, TryRotate)
ZX_PROPERTY(bool, tryInvert, TryInvert)
ZX_PROPERTY(bool, tryDownscale, TryDownscale)
#ifdef ZXING_EXPERIMENTAL_API
	ZX_PROPERTY(bool, tryDenoise, TryDenoise)
#endif
ZX_PROPERTY(bool, isPure, IsPure)
ZX_PROPERTY(bool, validateOptionalChecksum, ValidateOptionalChecksum)
ZX_PROPERTY(bool, returnErrors, ReturnErrors)
ZX_PROPERTY(int, minLineCount, MinLineCount)
ZX_PROPERTY(int, maxNumberOfSymbols, MaxNumberOfSymbols)

#undef ZX_PROPERTY

void ZXing_ReaderOptions_setFormats(ZXing_ReaderOptions* opts, const ZXing_BarcodeFormat* formats, int count)
{
	if (!formats || !count)
		return;
	if (count == -1) // determine count by looking for null terminator
		for (count = 0; formats[count] != ZXing_BarcodeFormat_None; ++count)
			;
	std::vector<BarcodeFormat> v((BarcodeFormat*)formats, (BarcodeFormat*)formats + count);
	opts->formats(std::move(v));
}

ZXing_BarcodeFormat* ZXing_ReaderOptions_getFormats(const ZXing_ReaderOptions* opts, int* outCount)
{
	return (ZXing_BarcodeFormat*)copy(opts->formats(), outCount);
}

#define ZX_ENUM_PROPERTY(TYPE, NAME, CAP_NAME) \
	ZXing_##TYPE ZXing_ReaderOptions_get##CAP_NAME(const ZXing_ReaderOptions* opts) { return static_cast<ZXing_##TYPE>(opts->NAME()); } \
	void ZXing_ReaderOptions_set##CAP_NAME(ZXing_ReaderOptions* opts, ZXing_##TYPE NAME) { opts->NAME(static_cast<TYPE>(NAME)); }

ZX_ENUM_PROPERTY(Binarizer, binarizer, Binarizer)
ZX_ENUM_PROPERTY(EanAddOnSymbol, eanAddOnSymbol, EanAddOnSymbol)
ZX_ENUM_PROPERTY(TextMode, textMode, TextMode)

#undef ZX_ENUM_PROPERTY

/*
 * MARK: - ReadBarcode.h
 */

ZXing_Barcodes* ZXing_ReadBarcodes(const ZXing_ImageView* iv, const ZXing_ReaderOptions* opts)
{
	ZX_CHECK(iv, "ImageView param is NULL")
	try {
		auto res = ReadBarcodes(*iv, opts ? *opts : ReaderOptions{});
		return res.empty() ? &emptyBarcodes : new Barcodes(std::move(res));
	}
	ZX_CATCH(NULL);
}


/*
 * MARK: - CreateBarcode.h
 */

ZXing_CreatorOptions* ZXing_CreatorOptions_new(ZXing_BarcodeFormat format)
{
	ZX_TRY(new CreatorOptions(static_cast<BarcodeFormat>(format)));
}

void ZXing_CreatorOptions_delete(ZXing_CreatorOptions* opts)
{
	delete opts;
}

#define ZX_ENUM_PROPERTY(TYPE, NAME, CAP_NAME) \
	ZXing_##TYPE ZXing_CreatorOptions_get##CAP_NAME(const ZXing_CreatorOptions* opts) { return transmute_cast<ZXing_##TYPE>(opts->NAME()); } \
	void ZXing_CreatorOptions_set##CAP_NAME(ZXing_CreatorOptions* opts, ZXing_##TYPE NAME) { opts->NAME(transmute_cast<TYPE>(NAME)); }

ZX_ENUM_PROPERTY(BarcodeFormat, format, Format)


#undef ZX_ENUM_PROPERTY

#define ZX_PROPERTY(TYPE, NAME, CAP_NAME) \
	TYPE ZXing_CreatorOptions_get##CAP_NAME(const ZXing_CreatorOptions* opts) { return opts->NAME(); } \
	void ZXing_CreatorOptions_set##CAP_NAME(ZXing_CreatorOptions* opts, TYPE NAME) { opts->NAME(NAME); }

#undef ZX_PROPERTY

char* ZXing_CreatorOptions_getOptions(const ZXing_CreatorOptions* opts)
{
	return copy(opts->options());
}

void ZXing_CreatorOptions_setOptions(ZXing_CreatorOptions* opts, const char* val)
{
	opts->options(val);
}


/*
 * MARK: - WriteBarcode.h
 */

ZXing_WriterOptions* ZXing_WriterOptions_new()
{
	ZX_TRY(new ZXing_WriterOptions());
}

void ZXing_WriterOptions_delete(ZXing_WriterOptions* opts)
{
	delete opts;
}

#define ZX_PROPERTY(TYPE, NAME, CAP_NAME) \
	TYPE ZXing_WriterOptions_get##CAP_NAME(const ZXing_WriterOptions* opts) { return opts->NAME(); } \
	void ZXing_WriterOptions_set##CAP_NAME(ZXing_WriterOptions* opts, TYPE NAME) { opts->NAME(NAME); }

ZX_PROPERTY(int, scale, Scale)
ZX_PROPERTY(int, rotate, Rotate)
ZX_PROPERTY(bool, addHRT, AddHRT)
ZX_PROPERTY(bool, addQuietZones, AddQuietZones)

#undef ZX_PROPERTY

ZXing_Barcode* ZXing_CreateBarcodeFromText(const char* data, int size, const ZXing_CreatorOptions* opts)
{
	ZX_CHECK(data && opts, "Data and/or options param in CreateBarcodeFromText is NULL")
	ZX_TRY(new Barcode(CreateBarcodeFromText({data, size ? static_cast<size_t>(size) : strlen(data)}, *opts));)
}

ZXing_Barcode* ZXing_CreateBarcodeFromBytes(const void* data, int size, const ZXing_CreatorOptions* opts)
{
	ZX_CHECK(data && size && opts, "Data and/or options param in CreateBarcodeFromBytes is NULL")
	ZX_TRY(new Barcode(CreateBarcodeFromBytes(data, size, *opts)))
}

char* ZXing_WriteBarcodeToSVG(const ZXing_Barcode* barcode, const ZXing_WriterOptions* opts)
{
	ZX_CHECK(barcode, "Barcode param in WriteBarcodeToSVG is NULL")
	ZX_TRY(copy(opts ? WriteBarcodeToSVG(*barcode, *opts) : WriteBarcodeToSVG(*barcode)))
}

ZXing_Image* ZXing_WriteBarcodeToImage(const ZXing_Barcode* barcode, const ZXing_WriterOptions* opts)
{
	ZX_CHECK(barcode, "Barcode param in WriteBarcodeToSVG is NULL")
	ZX_TRY(new Image(opts ? WriteBarcodeToImage(*barcode, *opts) : WriteBarcodeToImage(*barcode)))
}

/*
 * MARK: - ZXingC.h
 */

char* ZXing_LastErrorMsg()
{
	if (lastErrorMsg.empty())
		return NULL;

	return copy(std::exchange(lastErrorMsg, {}));
}

const char* ZXing_Version()
{
	return ZXING_VERSION_STR;
}

void ZXing_free(void* ptr)
{
	if (ptr != ZXing_Version())
		free(ptr);
}

} // extern "C"
