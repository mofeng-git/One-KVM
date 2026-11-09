/*
* Copyright 2017 Huy Cuong Nguyen
*/
// SPDX-License-Identifier: Apache-2.0

#include "MultiFormatWriter.h"

#include "BitMatrix.h"
#include "Utf.h"
#include "Version.h"

#if ZXING_ENABLE_AZTEC
#include "aztec/AZWriter.h"
#endif
#if ZXING_ENABLE_DATAMATRIX
#include "datamatrix/DMWriter.h"
#endif
#if ZXING_ENABLE_1D
#include "oned/ODCodabarWriter.h"
#include "oned/ODCode128Writer.h"
#include "oned/ODCode39Writer.h"
#include "oned/ODCode93Writer.h"
#include "oned/ODEAN13Writer.h"
#include "oned/ODEAN8Writer.h"
#include "oned/ODITFWriter.h"
#include "oned/ODUPCAWriter.h"
#include "oned/ODUPCEWriter.h"
#endif
#if ZXING_ENABLE_PDF417
#include "pdf417/PDFWriter.h"
#endif
#if ZXING_ENABLE_QRCODE
#include "qrcode/QRErrorCorrectionLevel.h"
#include "qrcode/QRWriter.h"
#endif

#include <stdexcept>

namespace ZXing {

BitMatrix
MultiFormatWriter::encode(const std::wstring& contents, int width, int height) const
{
	[[maybe_unused]] auto exec0 = [&](auto&& writer) {
		if (_margin >=0)
			writer.setMargin(_margin);
		return writer.encode(contents, width, height);
	};

#if ZXING_ENABLE_AZTEC
	auto AztecEccLevel = [&](Aztec::Writer& writer, int eccLevel) { writer.setEccPercent(eccLevel * 100 / 8); };
#endif
#if ZXING_ENABLE_PDF417
	auto Pdf417EccLevel = [&](Pdf417::Writer& writer, int eccLevel) { writer.setErrorCorrectionLevel(eccLevel); };
#endif
#if ZXING_ENABLE_QRCODE
	auto QRCodeEccLevel = [&](QRCode::Writer& writer, int eccLevel) {
		writer.setErrorCorrectionLevel(static_cast<QRCode::ErrorCorrectionLevel>(--eccLevel / 2));
	};
#endif

	[[maybe_unused]] auto exec1 = [&](auto&& writer, auto setEccLevel) {
		if (_encoding != CharacterSet::Unknown)
			writer.setEncoding(_encoding);
		if (_eccLevel >= 0 && _eccLevel <= 8)
			setEccLevel(writer, _eccLevel);
		return exec0(std::forward<decltype(writer)>(writer));
	};

	[[maybe_unused]] auto exec2 = [&](auto&& writer) {
		if (_encoding != CharacterSet::Unknown)
			writer.setEncoding(_encoding);
		return exec0(std::forward<decltype(writer)>(writer));
	};

	switch (_format) {
#if ZXING_ENABLE_AZTEC
	case BarcodeFormat::AztecCode: return exec1(Aztec::Writer(), AztecEccLevel);
#endif
#if ZXING_ENABLE_DATAMATRIX
	case BarcodeFormat::DataMatrix: return exec2(DataMatrix::Writer());
#endif
#if ZXING_ENABLE_PDF417
	case BarcodeFormat::PDF417: return exec1(Pdf417::Writer(), Pdf417EccLevel);
#endif
#if ZXING_ENABLE_QRCODE
	case BarcodeFormat::QRCode: return exec1(QRCode::Writer(), QRCodeEccLevel);
#endif
#if ZXING_ENABLE_1D
	case BarcodeFormat::Codabar: return exec0(OneD::CodabarWriter());
	case BarcodeFormat::Code39: return exec0(OneD::Code39Writer());
	case BarcodeFormat::Code93: return exec0(OneD::Code93Writer());
	case BarcodeFormat::Code128: return exec0(OneD::Code128Writer());
	case BarcodeFormat::EAN8: return exec0(OneD::EAN8Writer());
	case BarcodeFormat::EAN13: return exec0(OneD::EAN13Writer());
	case BarcodeFormat::ITF: return exec0(OneD::ITFWriter());
	case BarcodeFormat::UPCA: return exec0(OneD::UPCAWriter());
	case BarcodeFormat::UPCE: return exec0(OneD::UPCEWriter());
#endif
	default: throw std::invalid_argument("Unsupported format: " + ToString(_format));
	}
}

BitMatrix MultiFormatWriter::encode(const std::string& contents, int width, int height) const
{
	return encode(FromUtf8(contents), width, height);
}

} // ZXing
