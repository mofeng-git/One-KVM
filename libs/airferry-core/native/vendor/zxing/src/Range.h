/*
* Copyright 2022 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "ZXAlgorithms.h"

#include <cstdint>
#include <iterator>

namespace ZXing {

template <typename Iterator>
struct StrideIter
{
	Iterator pos;
	int stride;

	using iterator_category = std::random_access_iterator_tag;
	using difference_type   = typename std::iterator_traits<Iterator>::difference_type;
	using value_type        = typename std::iterator_traits<Iterator>::value_type;
	using pointer           = Iterator;
	using reference         = typename std::iterator_traits<Iterator>::reference;

	auto operator*() const { return *pos; }
	auto operator[](int i) const { return *(pos + i * stride); }
	StrideIter<Iterator>& operator++() { return pos += stride, *this; }
	StrideIter<Iterator> operator++(int) { auto temp = *this; ++*this; return temp; }
	bool operator==(const StrideIter<Iterator>& rhs) const { return pos == rhs.pos; }
	bool operator!=(const StrideIter<Iterator>& rhs) const { return pos != rhs.pos; }
	StrideIter<Iterator> operator+(int i) const { return {pos + i * stride, stride}; }
	StrideIter<Iterator> operator-(int i) const { return {pos - i * stride, stride}; }
	int operator-(const StrideIter<Iterator>& rhs) const { return narrow_cast<int>((pos - rhs.pos) / stride); }
};

template <typename Iterator>
StrideIter(const Iterator&, int) -> StrideIter<Iterator>;


template <typename Iterator>
struct Range
{
	Iterator _begin, _end;

	Range(Iterator b, Iterator e) : _begin(b), _end(e) {}

	template <typename C>
	Range(const C& c) : _begin(std::begin(c)), _end(std::end(c)) {}

	Iterator begin() const noexcept { return _begin; }
	Iterator end() const noexcept { return _end; }
	explicit operator bool() const { return begin() < end(); }
	int size() const { return narrow_cast<int>(end() - begin()); }
};

template <typename C>
Range(const C&) -> Range<typename C::const_iterator>;

/**
 * ArrayView is a lightweight, non-owning, non-mutable view over a contiguous sequence of elements.
 * Similar to std::span<const T>. See also Range template for general iterator use case.
 */
template <typename T>
class ArrayView
{
	const T* _data = nullptr;
	std::size_t _size = 0;

public:
	using value_type = T;
	using pointer = const value_type*;
	using const_pointer = const value_type*;
	using reference = const value_type&;
	using const_reference = const value_type&;
	using size_type = std::size_t;

	constexpr ArrayView() noexcept = default;

	constexpr ArrayView(pointer data, size_type size) noexcept : _data(data), _size(size) {}

	template <typename P, typename U = T,
			  typename = std::enable_if_t<
				  sizeof(U) == 1 && std::is_same_v<void, std::remove_cv_t<std::remove_reference_t<std::remove_pointer_t<P>>>>>>
	constexpr ArrayView(P data, size_type size) noexcept : _data(reinterpret_cast<pointer>(data)), _size(size)
	{}

	template <typename Container,
			  typename = std::enable_if_t<std::is_convertible_v<decltype(std::data(std::declval<Container&>())), const_pointer>>>
	constexpr ArrayView(const Container& c) noexcept : _data(std::data(c)), _size(std::size(c))
	{}

	constexpr pointer data() const noexcept { return _data; }
	constexpr size_type size() const noexcept { return _size; }
	constexpr bool empty() const noexcept { return _size == 0; }

	constexpr const_reference operator[](size_type index) const noexcept { return _data[index]; }

	constexpr pointer begin() const noexcept { return _data; }
	constexpr pointer end() const noexcept { return _data + _size; }

	constexpr ArrayView<T> subview(size_type pos, size_type len = size_type(-1)) const noexcept
	{
		if (pos > _size)
			return {};
		return {_data + pos, std::min(len, _size - pos)};
	}
};

using ByteView = ArrayView<uint8_t>;

} // namespace ZXing
