/*
* Copyright 2026 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "ZXAlgorithms.h"

#include <string>

namespace ZXing {

enum class AIFlag : char { None, GS1, AIM };

struct SymbologyIdentifier
{
	char code = 0, modifier = 0;
	signed char eciModifierOffset = 0;
	AIFlag aiFlag = AIFlag::None;

	std::string toString(bool hasECI = false) const
	{
		int modVal = (modifier >= 'A' ? modifier - 'A' + 10 : modifier - '0') + eciModifierOffset * hasECI;
		return code ? StrCat(']', code, static_cast<char>((modVal >= 10 ? 'A' - 10 : '0') + modVal)) : std::string();
	}
};

} // ZXing
