/*
* Copyright 2016 Nu-book Inc.
*/
// SPDX-License-Identifier: Apache-2.0

#include "TextEncoder.h"

#include "CharacterSet.h"
#include "ECI.h"
#include "Utf.h"
#include "ZXAlgorithms.h"
#ifdef ZXING_USE_ZINT
#include <zint.h>
#else
#include "libzueci/zueci.h"
#endif

#include <stdexcept>

namespace ZXing {

void TextEncoder::GetBytes(const std::string& str, CharacterSet charset, std::string& bytes)
{
	int eci = ToInt(ToECI(charset));
	const int str_len = narrow_cast<int>(str.length());
	int eci_len;

	if (eci == -1)
		eci = 899; // Binary

	bytes.clear();

#ifdef ZXING_USE_ZINT
	int error_number = ZBarcode_Dest_Len_ECI(eci, reinterpret_cast<const unsigned char *>(str.data()), str_len, &eci_len);
	if (error_number >= ZINT_ERROR) // Shouldn't happen
		throw std::logic_error("Internal error `ZBarcode_Dest_Len_ECI()`");

	bytes.resize(eci_len); // Sufficient but approximate length

	error_number = ZBarcode_UTF8_To_ECI(eci,reinterpret_cast<const unsigned char *>(str.data()), str_len,
										reinterpret_cast<unsigned char *>(bytes.data()), &eci_len);
	if (error_number >= ZINT_ERROR) {
		bytes.clear();
		throw std::invalid_argument("Unexpected charcode");
	}
#else
	int error_number = zueci_dest_len_eci(eci, reinterpret_cast<const unsigned char *>(str.data()), str_len, &eci_len);
	if (error_number >= ZUECI_ERROR) // Shouldn't happen
		throw std::logic_error("Internal error `zueci_dest_len_eci()`");

	bytes.resize(eci_len); // Sufficient but approximate length

	error_number = zueci_utf8_to_eci(eci, reinterpret_cast<const unsigned char *>(str.data()), str_len,
									 reinterpret_cast<unsigned char *>(bytes.data()), &eci_len);
	if (error_number >= ZUECI_ERROR) {
		bytes.clear();
		throw std::invalid_argument("Unexpected charcode");
	}
#endif // ZXING_USE_ZINT

	bytes.resize(eci_len); // Actual length
}

void TextEncoder::GetBytes(const std::wstring& str, CharacterSet charset, std::string& bytes)
{
	GetBytes(ToUtf8(str), charset, bytes);
}

} // ZXing
