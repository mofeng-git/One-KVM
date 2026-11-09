/*
 * Copyright 2026 Axel Waggershauser
 */
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include <version>

#if defined(__cpp_lib_print) && __cpp_lib_print >= 202207L

#include <print>

#else

#include <format>
#include <cstdio>
#include <utility>

namespace std {

template <class... Args>
inline void print(std::format_string<Args...> fmt, Args&&... args)
{
	auto s = std::format(fmt, std::forward<Args>(args)...);
	std::fwrite(s.data(), 1, s.size(), stdout);
}

template <class... Args>
inline void println(std::format_string<Args...> fmt, Args&&... args)
{
	std::print(fmt, std::forward<Args>(args)...);
	std::fputc('\n', stdout);
}

} // namespace std

#endif
