/*
 * Copyright 2026 Axel Waggershauser
 */
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include <version>

#if defined(__cpp_lib_generator)

#include <generator>

#else

#include <concepts>
#include <coroutine>
#include <optional>
#include <utility>

namespace std {

template <std::movable T>
class generator
{
public:
	struct promise_type
	{
		std::optional<T> value_;

		generator get_return_object() noexcept { return generator{handle_type::from_promise(*this)}; }

		static std::suspend_always initial_suspend() noexcept { return {}; }
		static std::suspend_always final_suspend() noexcept { return {}; }

		std::suspend_always yield_value(T&& value) noexcept
		{
			value_ = std::move(value);
			return {};
		}

		static void return_void() noexcept {}
		// Disallow co_await in generator coroutines.
		void await_transform() = delete;
		[[noreturn]] static void unhandled_exception() { throw; }
	};

	using handle_type = std::coroutine_handle<promise_type>;

	generator() noexcept = default;
	explicit generator(handle_type h) noexcept : h_(h) {}
	~generator()
	{
		if (h_)
			h_.destroy();
	}

	generator(generator&& other) noexcept : h_(other.h_) { other.h_ = nullptr; }

	generator& operator=(generator&& other) noexcept
	{
		if (this != &other) {
			if (h_)
				h_.destroy();
			h_ = other.h_;
			other.h_ = nullptr;
		}
		return *this;
	}

	generator(const generator&) = delete;
	generator& operator=(const generator&) = delete;

	class iterator
	{
	public:
		void operator++() { h_.resume(); }
		T&& operator*() const noexcept { return std::move(*h_.promise().value_); }
		bool operator==(std::default_sentinel_t) const noexcept { return !h_ || h_.done(); }

		explicit iterator(handle_type h) : h_(h) {}

	private:
		handle_type h_ = nullptr;
	};

	iterator begin()
	{
		if (h_)
			h_.resume();
		return iterator{h_};
	}

	std::default_sentinel_t end() noexcept { return {}; }

private:
	handle_type h_;
};

} // namespace std

#endif // __cpp_lib_generator

/*
usage example:

template <std::integral T>
std::generator<T> range(T first, const T last)
{
	while (first < last)
		co_yield first++;
}

int main()
{
	for (const char i : range(65, 91))
		std::print("{} ", i);
	std::println("");
}
*/
