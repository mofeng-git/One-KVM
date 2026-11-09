/*
 * Copyright 2026 Axel Waggershauser
 */
// SPDX-License-Identifier: Apache-2.0

#pragma once

// idea from <experimental/scope> (library fundamentals TS v3)

#include <type_traits>
#include <utility>

namespace std {

template <class F>
class scope_exit
{
public:
	explicit scope_exit(F&& f) noexcept(std::is_nothrow_move_constructible_v<F>) : func_(std::forward<F>(f)), active_(true) {}

	scope_exit(scope_exit&& other) noexcept(std::is_nothrow_move_constructible_v<F>)
		: func_(std::move(other.func_)), active_(std::exchange(other.active_, false))
	{}

	scope_exit(const scope_exit&) = delete;
	scope_exit& operator=(const scope_exit&) = delete;
	scope_exit& operator=(scope_exit&&) = delete;

	~scope_exit() noexcept(noexcept(std::declval<F&>()()))
	{
		if (active_)
			func_();
	}

	void release() noexcept { active_ = false; }

private:
	F func_;
	bool active_;
};

template <class F>
scope_exit(F) -> scope_exit<F>;

} // namespace std

/**
 * The SCOPE_EXIT macro is eliminating the need to give the object a name.
 * Example usage:
 *   SCOPE_EXIT([]{ printf("exiting scope"); });
 */

#define SCOPE_EXIT_CAT2(x, y) x##y
#define SCOPE_EXIT_CAT(x, y) SCOPE_EXIT_CAT2(x, y)
#define SCOPE_EXIT const auto SCOPE_EXIT_CAT(scopeExit_, __COUNTER__) = std::scope_exit
