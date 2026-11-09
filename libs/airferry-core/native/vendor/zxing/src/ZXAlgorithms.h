/*
* Copyright 2017 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "Error.h"

#include <algorithm>
#include <charconv>
#include <cstring>
#include <initializer_list>
#include <iterator>
#include <numeric>
#include <string>
#include <stdexcept>
#include <utility>
#include <vector>

namespace ZXing {

template <class T, class U>
constexpr T narrow_cast(U&& u) noexcept {
	return static_cast<T>(std::forward<U>(u));
}

template <typename Container, typename Value>
auto Find(Container& c, const Value& v) -> decltype(std::begin(c)) {
	return std::find(std::begin(c), std::end(c), v);
}

template <typename Container, typename Predicate>
auto FindIf(Container& c, Predicate p) -> decltype(std::begin(c)) {
	return std::find_if(std::begin(c), std::end(c), p);
}

template <typename Container, typename Value>
auto Contains(const Container& c, const Value& v) -> decltype(std::begin(c), bool()){
	return Find(c, v) != std::end(c);
}

template <typename ListType, typename Value>
auto Contains(const std::initializer_list<ListType>& c, const Value& v) -> decltype(std::begin(c), bool()){
	return Find(c, v) != std::end(c);
}

inline bool Contains(const char* str, char c) {
	return str && strchr(str, c) != nullptr;
}

inline bool Contains(std::string_view str, std::string_view substr) {
	return str.find(substr) != std::string_view::npos;
}

template <template <typename...> typename C, typename... Ts>
auto FirstOrDefault(C<Ts...>&& container)
{
	return container.empty() ? typename C<Ts...>::value_type() : std::move(container.front());
}

template <typename Iterator, typename Value = typename std::iterator_traits<Iterator>::value_type, typename Op = std::plus<Value>>
Value Reduce(Iterator b, Iterator e, Value v = Value{}, Op op = {}) {
	// std::reduce() first sounded like a better implementation because it is not implemented as a strict left-fold
	// operation, meaning the order of the op-application is not specified. This sounded like an optimization opportunity
	// but it turns out that for this use case it actually does not make a difference (falsepositives runtime). And
	// when tested with a large std::vector<uint16_t> and proper autovectorization (e.g. clang++ -O2) it turns out that
	// std::accumulate can be twice as fast as std::reduce.
	return std::accumulate(b, e, v, op);
}

template <typename Container, typename Value = typename Container::value_type, typename Op = std::plus<Value>>
Value Reduce(const Container& c, Value v = Value{}, Op op = {}) {
	return Reduce(std::begin(c), std::end(c), v, op);
}

// see C++20 ssize
template <class Container>
constexpr auto Size(const Container& c) -> decltype(c.size(), int()) {
	return narrow_cast<int>(c.size());
}

template <class T, std::size_t N>
constexpr int Size(T const (&)[N]) noexcept {
	return narrow_cast<int>(N);
}

inline constexpr int Size(const char* s) noexcept {
	return narrow_cast<int>(std::char_traits<char>::length(s));
}

inline constexpr int Size(char) noexcept { return 1; }

template <typename... Args>
std::string StrCat(Args&&... args)
{
	std::string res;
	res.reserve((Size(args) + ...));
	(res += ... += args);
	return res;
}

template <typename Container, typename Value>
int IndexOf(const Container& c, const Value& v) {
	auto i = Find(c, v);
	return i == std::end(c) ? -1 : narrow_cast<int>(std::distance(std::begin(c), i));
}

inline int IndexOf(const char* str, char c) {
	auto s = strchr(str, c);
	return s != nullptr ? narrow_cast<int>(s - str) : -1;
}

template <typename Container, typename Value, class UnaryOp>
Value TransformReduce(const Container& c, Value s, UnaryOp op) {
	for (const auto& v : c)
		s += op(v);
	return s;
}

template <typename T = char>
T ToDigit(int i)
{
	if (i < 0 || i > 9)
		throw FormatError("Invalid digit value");
	return static_cast<T>('0' + i);
}

template<typename T, typename = std::enable_if_t<std::is_integral_v<T>>>
std::string ToString(T val, int len)
{
	std::string result(len--, '0');
	if (val < 0)
		throw FormatError("Invalid value");
	for (; len >= 0 && val != 0; --len, val /= 10)
		result[len] = '0' + val % 10;
	if (val)
		throw FormatError("Invalid value");
	return result;
}

template<typename C, typename = std::enable_if_t<sizeof(typename C::value_type) == 1>>
std::string ToHex(const C& c)
{
	static constexpr char hex[] = "0123456789ABCDEF";

	const auto data = reinterpret_cast<const uint8_t*>(c.data());
	const auto size = Size(c);
	if (size == 0)
		return {};

	std::string res(size * 3 - 1, ' ');

	for (int i = 0; i < size; ++i) {
		res[i * 3 + 0] = hex[data[i] >> 4];
		res[i * 3 + 1] = hex[data[i] & 0x0F];
		if (i + 1 < size)
			res[i * 3 + 2] = ' ';
	}

	return res;
}

template <typename T>
std::vector<T> ToVector(T&& v)
{
	// simply constructing a vector via initializer_list does not work with move-only types
	std::vector<T> res;
	res.emplace_back(std::forward<T>(v));
	return res;
}

template <typename T, typename = std::enable_if_t<std::is_integral_v<T>>>
constexpr inline auto ToUnsigned(T v) noexcept
{
	return static_cast<std::make_unsigned_t<T>>(v);
}

template <class T>
constexpr std::string_view TypeName()
{
#ifdef __clang__
	std::string_view p = __PRETTY_FUNCTION__;
	return p.substr(40, p.size() - 40 - 1);
#elif defined(__GNUC__)
	std::string_view p = __PRETTY_FUNCTION__;
	return p.substr(55, p.find(';', 55) - 55);
#elif defined(_MSC_VER)
	std::string_view p = __FUNCSIG__;
	return p.substr(90, p.size() - 90 - 7);
#endif
}

template<typename T, typename = std::enable_if_t<std::is_arithmetic_v<T>>>
inline T FromString(std::string_view sv)
{
	T val = {};
	auto [ptr, ec] = std::from_chars(sv.data(), sv.data() + sv.size(), val);
	if (ec != std::errc() || ptr != sv.data() + sv.size())
		throw std::invalid_argument(StrCat("failed to parse '", TypeName<T>(), "' from '", sv, "'"));

	return val;
}

// Trim whitespace from both ends
inline std::string_view TrimWS(std::string_view sv, const char* ws = " \t\n\r")
{
	while (sv.size() && Contains(ws, sv.back()))
		sv.remove_suffix(1);
	while (sv.size() && Contains(ws, sv.front()))
		sv.remove_prefix(1);
	return sv.empty() ? std::string_view() : sv;
}

// Split string into tokens based on delimiters and call callback for each token
template <typename FUNC>
inline void ForEachToken(std::string_view str, std::string_view delimiters, FUNC&& callback)
{
	std::size_t pos = 0;
	while (pos < str.size()) {
		auto const next_pos = str.find_first_of(delimiters, pos);
		callback(str.substr(pos, next_pos - pos));
		pos = next_pos == std::string_view::npos ? str.size() : next_pos + 1;
	}
}

inline bool IsEqualIgnoreCase(std::string_view a, std::string_view b)
{
	return a.size() == b.size()
		   && std::equal(a.begin(), a.end(), b.begin(), [](uint8_t a, uint8_t b) { return std::tolower(a) == std::tolower(b); });
}

// Compare two strings ignoring case and specified whitespace characters
inline bool IsEqualIgnoreCaseAnd(std::string_view a, std::string_view b, const char* ws)
{
	auto i = a.begin(), j = b.begin();
	while (i != a.end() && j != b.end()) {
		if (Contains(ws, *i))
			++i;
		else if (Contains(ws, *j))
			++j;
		else if (std::tolower(static_cast<uint8_t>(*i)) != std::tolower(static_cast<uint8_t>(*j)))
			return false;
		else
			++i, ++j;
	}
	return i == a.end() && j == b.end();
}

template <typename T>
void UpdateMin(T& min, T val)
{
	min = std::min(min, val);
}

template <typename T>
void UpdateMax(T& max, T val)
{
	max = std::max(max, val);
}

template <typename T>
void UpdateMinMax(T& min, T& max, T val)
{
	min = std::min(min, val);
	max = std::max(max, val);

	// Note: the above code is not equivalent to
	//    if (val < min)        min = val;
	//    else if (val > max)   max = val;
	// It is basically the same but without the 'else'. For the 'else'-variant to work,
	// both min and max have to be initialized with a value that is part of the sequence.
	// Also it turns out clang and gcc can vectorize the code above but not the code below.
}

inline uint32_t ReverseBits32(uint32_t v)
{
	v = ((v >> 1) & 0x55555555) | ((v & 0x55555555) << 1);
	// swap consecutive pairs
	v = ((v >> 2) & 0x33333333) | ((v & 0x33333333) << 2);
	// swap nibbles ...
	v = ((v >> 4) & 0x0F0F0F0F) | ((v & 0x0F0F0F0F) << 4);
	// swap bytes
	v = ((v >> 8) & 0x00FF00FF) | ((v & 0x00FF00FF) << 8);
	// swap 2-byte long pairs
	v = (v >> 16) | (v << 16);
	return v;
}

// use to avoid "load of misaligned address" when using a simple type cast
template <typename T, typename = std::enable_if_t<std::is_integral_v<T>>>
T LoadU(const void* ptr)
{
	T res;
	memcpy(&res, ptr, sizeof(T));
	return res;
}

} // ZXing

#ifndef __cpp_lib_to_underlying

namespace std {

template <typename E, typename = std::enable_if_t<std::is_enum_v<E>>>
constexpr std::underlying_type_t<E> to_underlying(E e) noexcept
{
	return static_cast<std::underlying_type_t<E>>(e);
}

} // namespace std

#endif // __cpp_lib_to_underlying
