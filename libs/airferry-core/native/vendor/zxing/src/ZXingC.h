/*
* Copyright 2023 siiky
* Copyright 2023 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#ifndef _ZXING_C_H
#define _ZXING_C_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus

#include "ZXingCpp.h"

typedef ZXing::Barcode ZXing_Barcode;
typedef ZXing::Barcodes ZXing_Barcodes;
typedef ZXing::ImageView ZXing_ImageView;
typedef ZXing::Image ZXing_Image;
typedef ZXing::ReaderOptions ZXing_ReaderOptions;
typedef ZXing::CreatorOptions ZXing_CreatorOptions;
typedef ZXing::WriterOptions ZXing_WriterOptions;

extern "C"
{
#else

typedef struct ZXing_Barcode ZXing_Barcode;
typedef struct ZXing_Barcodes ZXing_Barcodes;
typedef struct ZXing_ImageView ZXing_ImageView;
typedef struct ZXing_Image ZXing_Image;
typedef struct ZXing_ReaderOptions ZXing_ReaderOptions;
typedef struct ZXing_CreatorOptions ZXing_CreatorOptions;
typedef struct ZXing_WriterOptions ZXing_WriterOptions;

#include "BarcodeFormat.h"

#endif

/*
 * MARK: - ImageView.h
 */

typedef enum {
	ZXing_ImageFormat_None = 0,
	ZXing_ImageFormat_Lum = 0x01000000,
	ZXing_ImageFormat_LumA = 0x02000000,
	ZXing_ImageFormat_RGB = 0x03000102,
	ZXing_ImageFormat_BGR = 0x03020100,
	ZXing_ImageFormat_RGBA = 0x04000102,
	ZXing_ImageFormat_ARGB = 0x04010203,
	ZXing_ImageFormat_BGRA = 0x04020100,
	ZXing_ImageFormat_ABGR = 0x04030201,
} ZXing_ImageFormat;

ZXing_ImageView* ZXing_ImageView_new(const uint8_t* data, int width, int height, ZXing_ImageFormat format, int rowStride,
									 int pixStride);
ZXing_ImageView* ZXing_ImageView_new_checked(const uint8_t* data, int size, int width, int height, ZXing_ImageFormat format,
											 int rowStride, int pixStride);
void ZXing_ImageView_delete(ZXing_ImageView* iv);

void ZXing_ImageView_crop(ZXing_ImageView* iv, int left, int top, int width, int height);
void ZXing_ImageView_rotate(ZXing_ImageView* iv, int degree);

void ZXing_Image_delete(ZXing_Image* img);

const uint8_t* ZXing_Image_data(const ZXing_Image* img);
int ZXing_Image_width(const ZXing_Image* img);
int ZXing_Image_height(const ZXing_Image* img);
ZXing_ImageFormat ZXing_Image_format(const ZXing_Image* img);

/*
 * MARK: - BarcodeFormat.h
 */

typedef enum
{
	ZXing_BarcodeFormat_Invalid = ZX_BCF_ID(0xff, 0xff), /* return value when BarcodeFormatsFromString() throws */
#define X(NAME, SYM, VAR, FLAGS, ZINT, ENABLED, HRI) ZXing_BarcodeFormat_##NAME = ZX_BCF_ID(SYM, VAR),
	ZX_BCF_LIST(X)
#undef X
} ZXing_BarcodeFormat;

ZXing_BarcodeFormat ZXing_BarcodeFormatSymbology(ZXing_BarcodeFormat format);
ZXing_BarcodeFormat ZXing_BarcodeFormatFromString(const char* str);
char* ZXing_BarcodeFormatToString(ZXing_BarcodeFormat format);

ZXing_BarcodeFormat* ZXing_BarcodeFormatsList(ZXing_BarcodeFormat filter, int* outCount);
ZXing_BarcodeFormat* ZXing_BarcodeFormatsFromString(const char* str, int* outCount);
char* ZXing_BarcodeFormatsToString(const ZXing_BarcodeFormat* formats, int count);


/*
 * MARK: - Barcode.h
 */

typedef enum
{
	ZXing_ContentType_Text,
	ZXing_ContentType_Binary,
	ZXing_ContentType_Mixed,
	ZXing_ContentType_GS1,
	ZXing_ContentType_ISO15434,
	ZXing_ContentType_UnknownECI
} ZXing_ContentType;

typedef enum
{
	ZXing_ErrorType_None,
	ZXing_ErrorType_Format,
	ZXing_ErrorType_Checksum,
	ZXing_ErrorType_Unsupported
} ZXing_ErrorType;

char* ZXing_ContentTypeToString(ZXing_ContentType type);

typedef struct ZXing_PointI
{
	int x, y;
} ZXing_PointI;

typedef struct ZXing_Position
{
	ZXing_PointI topLeft, topRight, bottomRight, bottomLeft;
} ZXing_Position;

char* ZXing_PositionToString(ZXing_Position position);

bool ZXing_Barcode_isValid(const ZXing_Barcode* barcode);
ZXing_ErrorType ZXing_Barcode_errorType(const ZXing_Barcode* barcode);
char* ZXing_Barcode_errorMsg(const ZXing_Barcode* barcode);
ZXing_BarcodeFormat ZXing_Barcode_format(const ZXing_Barcode* barcode);
ZXing_BarcodeFormat ZXing_Barcode_symbology(const ZXing_Barcode* barcode);
ZXing_ContentType ZXing_Barcode_contentType(const ZXing_Barcode* barcode);
uint8_t* ZXing_Barcode_bytes(const ZXing_Barcode* barcode, int* len);
uint8_t* ZXing_Barcode_bytesECI(const ZXing_Barcode* barcode, int* len);
char* ZXing_Barcode_text(const ZXing_Barcode* barcode);
char* ZXing_Barcode_symbologyIdentifier(const ZXing_Barcode* barcode);
ZXing_Position ZXing_Barcode_position(const ZXing_Barcode* barcode);
int ZXing_Barcode_orientation(const ZXing_Barcode* barcode);
bool ZXing_Barcode_hasECI(const ZXing_Barcode* barcode);
bool ZXing_Barcode_isInverted(const ZXing_Barcode* barcode);
bool ZXing_Barcode_isMirrored(const ZXing_Barcode* barcode);
int ZXing_Barcode_lineCount(const ZXing_Barcode* barcode);
int ZXing_Barcode_sequenceIndex(const ZXing_Barcode* barcode);
int ZXing_Barcode_sequenceSize(const ZXing_Barcode* barcode);
char* ZXing_Barcode_sequenceId(const ZXing_Barcode* barcode);
char* ZXing_Barcode_extra(const ZXing_Barcode* barcode, const char* key); /* key can be NULL */

void ZXing_Barcode_delete(ZXing_Barcode* barcode);
void ZXing_Barcodes_delete(ZXing_Barcodes* barcodes);

int ZXing_Barcodes_size(const ZXing_Barcodes* barcodes);
const ZXing_Barcode* ZXing_Barcodes_at(const ZXing_Barcodes* barcodes, int i);
ZXing_Barcode* ZXing_Barcodes_move(ZXing_Barcodes* barcodes, int i);

/*
 * MARK: - ReaderOptions.h
 */

typedef enum
{
	ZXing_Binarizer_LocalAverage,
	ZXing_Binarizer_GlobalHistogram,
	ZXing_Binarizer_FixedThreshold,
	ZXing_Binarizer_BoolCast,
} ZXing_Binarizer;

typedef enum
{
	ZXing_EanAddOnSymbol_Ignore,
	ZXing_EanAddOnSymbol_Read,
	ZXing_EanAddOnSymbol_Require,
} ZXing_EanAddOnSymbol;

typedef enum
{
	ZXing_TextMode_Plain,
	ZXing_TextMode_ECI,
	ZXing_TextMode_HRI,
	ZXing_TextMode_Escaped,
	ZXing_TextMode_Hex,
	ZXing_TextMode_HexECI,
} ZXing_TextMode;

ZXing_ReaderOptions* ZXing_ReaderOptions_new();
void ZXing_ReaderOptions_delete(ZXing_ReaderOptions* opts);

void ZXing_ReaderOptions_setTryHarder(ZXing_ReaderOptions* opts, bool tryHarder);
void ZXing_ReaderOptions_setTryRotate(ZXing_ReaderOptions* opts, bool tryRotate);
void ZXing_ReaderOptions_setTryInvert(ZXing_ReaderOptions* opts, bool tryInvert);
void ZXing_ReaderOptions_setTryDownscale(ZXing_ReaderOptions* opts, bool tryDownscale);
#ifdef ZXING_EXPERIMENTAL_API
	void ZXing_ReaderOptions_setTryDenoise(ZXing_ReaderOptions* opts, bool tryDenoise);
#endif
void ZXing_ReaderOptions_setIsPure(ZXing_ReaderOptions* opts, bool isPure);
void ZXing_ReaderOptions_setValidateOptionalChecksum(ZXing_ReaderOptions* opts, bool validateOptionalChecksum);
void ZXing_ReaderOptions_setReturnErrors(ZXing_ReaderOptions* opts, bool returnErrors);
void ZXing_ReaderOptions_setFormats(ZXing_ReaderOptions* opts, const ZXing_BarcodeFormat* formats, int count);
void ZXing_ReaderOptions_setBinarizer(ZXing_ReaderOptions* opts, ZXing_Binarizer binarizer);
void ZXing_ReaderOptions_setEanAddOnSymbol(ZXing_ReaderOptions* opts, ZXing_EanAddOnSymbol eanAddOnSymbol);
void ZXing_ReaderOptions_setTextMode(ZXing_ReaderOptions* opts, ZXing_TextMode textMode);
void ZXing_ReaderOptions_setMinLineCount(ZXing_ReaderOptions* opts, int n);
void ZXing_ReaderOptions_setMaxNumberOfSymbols(ZXing_ReaderOptions* opts, int n);

bool ZXing_ReaderOptions_getTryHarder(const ZXing_ReaderOptions* opts);
bool ZXing_ReaderOptions_getTryRotate(const ZXing_ReaderOptions* opts);
bool ZXing_ReaderOptions_getTryInvert(const ZXing_ReaderOptions* opts);
bool ZXing_ReaderOptions_getTryDownscale(const ZXing_ReaderOptions* opts);
#ifdef ZXING_EXPERIMENTAL_API
	bool ZXing_ReaderOptions_getTryDenoise(const ZXing_ReaderOptions* opts);
#endif
bool ZXing_ReaderOptions_getIsPure(const ZXing_ReaderOptions* opts);
bool ZXing_ReaderOptions_getValidateOptionalChecksum(const ZXing_ReaderOptions* opts);
bool ZXing_ReaderOptions_getReturnErrors(const ZXing_ReaderOptions* opts);
ZXing_BarcodeFormat* ZXing_ReaderOptions_getFormats(const ZXing_ReaderOptions* opts, int* outCount);
ZXing_Binarizer ZXing_ReaderOptions_getBinarizer(const ZXing_ReaderOptions* opts);
ZXing_EanAddOnSymbol ZXing_ReaderOptions_getEanAddOnSymbol(const ZXing_ReaderOptions* opts);
ZXing_TextMode ZXing_ReaderOptions_getTextMode(const ZXing_ReaderOptions* opts);
int ZXing_ReaderOptions_getMinLineCount(const ZXing_ReaderOptions* opts);
int ZXing_ReaderOptions_getMaxNumberOfSymbols(const ZXing_ReaderOptions* opts);

/*
 * MARK: - ReadBarcode.h
 */

/** Note: opts is optional, i.e. it can be NULL, which will imply default settings. */
ZXing_Barcodes* ZXing_ReadBarcodes(const ZXing_ImageView* iv, const ZXing_ReaderOptions* opts);


/*
 * MARK: - CreateBarcode.h
 */

ZXing_CreatorOptions* ZXing_CreatorOptions_new(ZXing_BarcodeFormat format);
void ZXing_CreatorOptions_delete(ZXing_CreatorOptions* opts);

void ZXing_CreatorOptions_setFormat(ZXing_CreatorOptions* opts, ZXing_BarcodeFormat format);
ZXing_BarcodeFormat ZXing_CreatorOptions_getFormat(const ZXing_CreatorOptions* opts);
void ZXing_CreatorOptions_setOptions(ZXing_CreatorOptions* opts, const char* options);
char* ZXing_CreatorOptions_getOptions(const ZXing_CreatorOptions* opts);

ZXing_Barcode* ZXing_CreateBarcodeFromText(const char* data, int size, const ZXing_CreatorOptions* opts);
ZXing_Barcode* ZXing_CreateBarcodeFromBytes(const void* data, int size, const ZXing_CreatorOptions* opts);

/*
 * MARK: - WriteBarcode.h
 */

ZXing_WriterOptions* ZXing_WriterOptions_new();
void ZXing_WriterOptions_delete(ZXing_WriterOptions* opts);

void ZXing_WriterOptions_setScale(ZXing_WriterOptions* opts, int scale);
int ZXing_WriterOptions_getScale(const ZXing_WriterOptions* opts);

void ZXing_WriterOptions_setRotate(ZXing_WriterOptions* opts, int rotate);
int ZXing_WriterOptions_getRotate(const ZXing_WriterOptions* opts);

void ZXing_WriterOptions_setAddHRT(ZXing_WriterOptions* opts, bool addHRT);
bool ZXing_WriterOptions_getAddHRT(const ZXing_WriterOptions* opts);

void ZXing_WriterOptions_setAddQuietZones(ZXing_WriterOptions* opts, bool addQuietZones);
bool ZXing_WriterOptions_getAddQuietZones(const ZXing_WriterOptions* opts);


/** Note: opts is optional, i.e. it can be NULL, which will imply default settings. */
char* ZXing_WriteBarcodeToSVG(const ZXing_Barcode* barcode, const ZXing_WriterOptions* opts);
ZXing_Image* ZXing_WriteBarcodeToImage(const ZXing_Barcode* barcode, const ZXing_WriterOptions* opts);


/* ZXing_LastErrorMsg() returns NULL in case there is no last error and a copy of the string otherwise. */
char* ZXing_LastErrorMsg();

const char* ZXing_Version();

void ZXing_free(void* ptr);

#ifdef __cplusplus
}
#endif

#endif /* _ZXING_C_H */
