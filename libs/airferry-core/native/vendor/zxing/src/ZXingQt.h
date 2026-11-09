/*
 * Copyright 2020 Axel Waggershauser
 */
// SPDX-License-Identifier: Apache-2.0

#pragma once

#include "ZXingCpp.h"

#include <QImage>
#include <QDebug>
#include <QMetaType>
#include <QPoint>
#include <QList>
#include <QObject>
#include <QScopeGuard>
#include <QThreadPool>
#include <QtQmlIntegration/qqmlintegration.h>

#ifdef QT_MULTIMEDIA_LIB
#include <QVideoFrame>
#include <QVideoSink>
#include <QElapsedTimer>
#endif

// This is some sample code to start a discussion about how a minimal and header-only Qt wrapper/helper could look like.

namespace ZXingQt {

Q_NAMESPACE

inline QString Version()
{
	return QString::fromStdString(ZXing::Version());
}

namespace Detail {

template<typename Cout, typename Cin>
inline Cout transcode(Cin&& in)
{
	Cout out;
	out.reserve(in.size());
	for (auto&& v : in)
		out.push_back(static_cast<typename Cout::value_type>(std::move(v)));
	return out;
}

inline std::string_view qba2sv(const QByteArray& ba) noexcept
{
	return std::string_view(ba.constData(), static_cast<size_t>(ba.size()));
}

} // namespace Detail

// MARK: - Enums and Types

enum class BarcodeFormat : unsigned int
{
#define X(NAME, SYM, VAR, FLAGS, ZINT, ENABLED, HRI) NAME = ZX_BCF_ID(SYM, VAR),
	ZX_BCF_LIST(X)
#undef X
};

enum class ContentType { Text, Binary, Mixed, GS1, ISO15434, UnknownECI };
enum class TextMode { Plain, ECI, HRI, Escaped, Hex, HexECI };
enum class Binarizer { LocalAverage, GlobalHistogram, FixedThreshold, BoolCast };

Q_ENUM_NS(BarcodeFormat)
Q_ENUM_NS(ContentType)
Q_ENUM_NS(TextMode)
Q_ENUM_NS(Binarizer)

using BarcodeFormats = QList<BarcodeFormat>;

using ZXing::ReaderOptions;
using ZXing::WriterOptions;

inline BarcodeFormat BarcodeFormatFromString(QStringView str)
{
	return static_cast<BarcodeFormat>(ZXing::BarcodeFormatFromString(Detail::qba2sv(str.toUtf8())));
}

inline BarcodeFormats BarcodeFormatsFromString(QStringView str)
{
	return Detail::transcode<BarcodeFormats>(ZXing::BarcodeFormatsFromString(Detail::qba2sv(str.toUtf8())));
}

inline BarcodeFormats ListBarcodeFormats(BarcodeFormat filter = BarcodeFormat::None)
{
	return Detail::transcode<BarcodeFormats>(ZXing::BarcodeFormats::list(
		filter == BarcodeFormat::None ? ZXing::BarcodeFormats() : static_cast<ZXing::BarcodeFormat>(filter)));
}

#define ZQ_DECL_TO_STRING(TYPE) \
	inline QString ToString(::ZXingQt::TYPE v) \
	{ \
		return QString::fromStdString(::ZXing::ToString(static_cast<::ZXing::TYPE>(v))); \
	}

ZQ_DECL_TO_STRING(BarcodeFormat)
ZQ_DECL_TO_STRING(ContentType)
#undef ZQ_DECL_TO_STRING

// template <typename T, typename = decltype(ToString(T()))>
// QDebug operator<<(QDebug dbg, const T& v)
// {
// 	return dbg.noquote() << ToString(v);
// }

class Error : private ZXing::Error
{
	Q_GADGET

	Q_PROPERTY(Type type READ type)
	Q_PROPERTY(QString message READ message)
	Q_PROPERTY(QString location READ location)

	friend QString ToString(const Error& err);

public:
	enum class Type { None, Format, Checksum, Unsupported };
	Q_ENUM(Type)

	Error() = default;
	explicit Error(const ZXing::Error& e) : ZXing::Error(e) {}

	Type type() const { return static_cast<Type>(ZXing::Error::type()); }
	QString message() const { return QString::fromStdString(ZXing::Error::msg()); }
	QString location() const { return QString::fromStdString(ZXing::Error::location()); }

	using ZXing::Error::operator bool;

	bool operator==(const Error& o) const { return ZXing::Error::operator==(o); }
};

inline QString ToString(const Error& err)
{
	return QString::fromStdString(ZXing::ToString(static_cast<ZXing::Error>(err)));
}

class Position : public ZXing::Quadrilateral<QPoint>
{
	Q_GADGET

	Q_PROPERTY(QPoint topLeft READ topLeft)
	Q_PROPERTY(QPoint topRight READ topRight)
	Q_PROPERTY(QPoint bottomRight READ bottomRight)
	Q_PROPERTY(QPoint bottomLeft READ bottomLeft)
	Q_PROPERTY(QPoint center READ center)

	using Base = ZXing::Quadrilateral<QPoint>;

public:
	using Base::Base;

	QPoint center() const { return std::accumulate(this->begin(), this->end(), QPoint(0, 0)) / 4; }
};

// MARK: - Barcode

class Barcode : private ZXing::Barcode
{
	Q_GADGET

	Q_PROPERTY(BarcodeFormat format READ format)
	Q_PROPERTY(BarcodeFormat symbology READ symbology)
	Q_PROPERTY(QString text READ text)
	Q_PROPERTY(QByteArray bytes READ bytes)
	Q_PROPERTY(QByteArray bytesECI READ bytesECI)
	Q_PROPERTY(bool isValid READ isValid)
	Q_PROPERTY(Error error READ error)
	Q_PROPERTY(ContentType contentType READ contentType)
	Q_PROPERTY(bool hasECI READ hasECI)
	Q_PROPERTY(Position position READ position)
	Q_PROPERTY(int orientation READ orientation)
	Q_PROPERTY(bool isMirrored READ isMirrored)
	Q_PROPERTY(bool isInverted READ isInverted)
	Q_PROPERTY(QString symbologyIdentifier READ symbologyIdentifier)
	Q_PROPERTY(int sequenceSize READ sequenceSize)
	Q_PROPERTY(int sequenceIndex READ sequenceIndex)
	Q_PROPERTY(QString sequenceId READ sequenceId)
	Q_PROPERTY(bool isLastInSequence READ isLastInSequence)
	Q_PROPERTY(bool isPartOfSequence READ isPartOfSequence)
	Q_PROPERTY(int lineCount READ lineCount)

public:
	Barcode() = default; // required for qmetatype machinery

	explicit Barcode(ZXing::Barcode&& r) : ZXing::Barcode(std::move(r)) {}

	using ZXing::Barcode::isValid;

	BarcodeFormat format() const { return static_cast<BarcodeFormat>(ZXing::Barcode::format()); }
	BarcodeFormat symbology() const { return static_cast<BarcodeFormat>(ZXing::Barcode::symbology()); }
	ContentType contentType() const { return static_cast<ContentType>(ZXing::Barcode::contentType()); }

	Error error() const { return Error(ZXing::Barcode::error()); }

	QString text() const { return QString::fromStdString(ZXing::Barcode::text()); }
	QString text(TextMode mode) const { return QString::fromStdString(ZXing::Barcode::text(static_cast<ZXing::TextMode>(mode))); }

	QByteArray bytes() const
	{
		return QByteArray(reinterpret_cast<const char*>(ZXing::Barcode::bytes().data()), std::size(ZXing::Barcode::bytes()));
	}

	QByteArray bytesECI() const
	{
		auto b = ZXing::Barcode::bytesECI();
		return QByteArray(reinterpret_cast<const char*>(b.data()), b.size());
	}

	using ZXing::Barcode::hasECI;

	Position position() const
	{
		auto& pos = ZXing::Barcode::position();
		auto qp = [&pos](int i) { return QPoint(pos[i].x, pos[i].y); };
		return {qp(0), qp(1), qp(2), qp(3)};
	}

	using ZXing::Barcode::orientation;
	using ZXing::Barcode::isMirrored;
	using ZXing::Barcode::isInverted;

	QString symbologyIdentifier() const { return QString::fromStdString(ZXing::Barcode::symbologyIdentifier()); }

	using ZXing::Barcode::sequenceSize;
	using ZXing::Barcode::sequenceIndex;
	using ZXing::Barcode::isLastInSequence;
	using ZXing::Barcode::isPartOfSequence;
	QString sequenceId() const { return QString::fromStdString(ZXing::Barcode::sequenceId()); }

	QString extra(QStringView key = {}) const { return QString::fromStdString(ZXing::Barcode::extra(Detail::qba2sv(key.toUtf8()))); }

	using ZXing::Barcode::lineCount;

	QString toSVG(const WriterOptions& options = {}) const
	{
		return QString::fromStdString(ZXing::WriteBarcodeToSVG(*this, options));
	}

	QImage toImage(const WriterOptions& options = {}) const
	{
		auto img = ZXing::WriteBarcodeToImage(*this, options);
		return QImage(img.data(), img.width(), img.height(), img.width(), QImage::Format::Format_Grayscale8).copy();
	}

	QImage symbol() const
	{
		auto img = ZXing::Barcode::symbol();
		return img.data() ? QImage(img.data(), img.width(), img.height(), img.width(), QImage::Format::Format_Grayscale8).copy() : QImage();
	}

	bool operator==(const Barcode& o) const { return ZXing::Barcode::operator==(o); }

	static Barcode fromText(QStringView text, BarcodeFormat format, QStringView options = {})
	{
		auto opts = ZXing::CreatorOptions(static_cast<ZXing::BarcodeFormat>(format), options.toUtf8().toStdString());
		return Barcode(ZXing::CreateBarcodeFromText(Detail::qba2sv(text.toUtf8()), opts));
	}

	static Barcode fromBytes(const QByteArray& bytes, BarcodeFormat format, QStringView options = {})
	{
		auto opts = ZXing::CreatorOptions(static_cast<ZXing::BarcodeFormat>(format), options.toUtf8().toStdString());
		return Barcode(ZXing::CreateBarcodeFromBytes(bytes, opts));
	}

};

// MARK: - Read

inline QList<Barcode> ReadBarcodes(const QImage& img, const ReaderOptions& opts = {})
{
	using namespace ZXing;

	auto ImgFmtFromQImg = [](const QImage& img) {
		switch (img.format()) {
		case QImage::Format_ARGB32:
		case QImage::Format_RGB32:
#if Q_BYTE_ORDER == Q_LITTLE_ENDIAN
			return ImageFormat::BGRA;
#else
			return ImageFormat::ARGB;
#endif
		case QImage::Format_RGB888: return ImageFormat::RGB;
		case QImage::Format_RGBX8888:
		case QImage::Format_RGBA8888: return ImageFormat::RGBA;
		case QImage::Format_Grayscale8: return ImageFormat::Lum;
		default: return ImageFormat::None;
		}
	};

	auto exec = [&](const QImage& img) {
		return Detail::transcode<QList<Barcode>>(ZXing::ReadBarcodes(
			{img.bits(), img.width(), img.height(), ImgFmtFromQImg(img), static_cast<int>(img.bytesPerLine())}, opts));
	};

	return ImgFmtFromQImg(img) == ImageFormat::None ? exec(img.convertToFormat(QImage::Format_Grayscale8)) : exec(img);
}

inline Barcode ReadBarcode(const QImage& img, const ReaderOptions& opts = {})
{
	auto res = ReadBarcodes(img, ReaderOptions(opts).setMaxNumberOfSymbols(1));
	return !res.isEmpty() ? res.takeFirst() : Barcode();
}

#ifdef QT_MULTIMEDIA_LIB
inline QList<Barcode> ReadBarcodes(const QVideoFrame& frame, const ReaderOptions& opts = {})
{
	using namespace ZXing;

	ImageFormat fmt = ImageFormat::None;
	int pixStride = 0;
	int pixOffset = 0;

	switch (frame.pixelFormat()) {
	case QVideoFrameFormat::Format_ARGB8888:
	case QVideoFrameFormat::Format_ARGB8888_Premultiplied:
	case QVideoFrameFormat::Format_XRGB8888: fmt = ImageFormat::ARGB; break;

	case QVideoFrameFormat::Format_BGRA8888:
	case QVideoFrameFormat::Format_BGRA8888_Premultiplied:
	case QVideoFrameFormat::Format_BGRX8888: fmt = ImageFormat::BGRA; break;

	case QVideoFrameFormat::Format_ABGR8888:
	case QVideoFrameFormat::Format_XBGR8888: fmt = ImageFormat::ABGR; break;

	case QVideoFrameFormat::Format_RGBA8888:
	case QVideoFrameFormat::Format_RGBX8888: fmt = ImageFormat::RGBA; break;

	case QVideoFrameFormat::Format_P010:
	case QVideoFrameFormat::Format_P016: fmt = ImageFormat::Lum, pixStride = 1; break;

	case QVideoFrameFormat::Format_YUV420P:
	case QVideoFrameFormat::Format_YUV422P:
	case QVideoFrameFormat::Format_NV12:
	case QVideoFrameFormat::Format_NV21:
	case QVideoFrameFormat::Format_IMC1:
	case QVideoFrameFormat::Format_IMC2:
	case QVideoFrameFormat::Format_IMC3:
	case QVideoFrameFormat::Format_IMC4:
	case QVideoFrameFormat::Format_YV12: fmt = ImageFormat::Lum; break;

	case QVideoFrameFormat::Format_AYUV_Premultiplied:
	case QVideoFrameFormat::Format_AYUV: fmt = ImageFormat::Lum, pixStride = 4, pixOffset = 1; break;
	case QVideoFrameFormat::Format_UYVY: fmt = ImageFormat::Lum, pixStride = 2, pixOffset = 1; break;
	case QVideoFrameFormat::Format_YUYV: fmt = ImageFormat::Lum, pixStride = 2; break;

	case QVideoFrameFormat::Format_Y8: fmt = ImageFormat::Lum; break;
	case QVideoFrameFormat::Format_Y16: fmt = ImageFormat::Lum, pixStride = 2, pixOffset = 1; break;

	default: break;
	}

	if (fmt != ImageFormat::None) {
		auto img = frame; // shallow copy just get access to non-const map() function
		if (!img.isValid() || !img.map(QVideoFrame::ReadOnly)) {
			qWarning() << "invalid QVideoFrame: could not map memory";
			return {};
		}
		QScopeGuard unmap([&] { img.unmap(); });

		return Detail::transcode<QList<Barcode>>(ZXing::ReadBarcodes(
			{img.bits(0) + pixOffset, img.width(), img.height(), fmt, img.bytesPerLine(0), pixStride}, opts));
	}
	else {
		auto qimg = frame.toImage();
		if (qimg.format() != QImage::Format_Invalid)
			return ReadBarcodes(qimg, opts);
		qWarning() << "failed to convert QVideoFrame to QImage";
		return {};
	}
}

inline Barcode ReadBarcode(const QVideoFrame& frame, const ReaderOptions& opts = {})
{
	auto res = ReadBarcodes(frame, ReaderOptions(opts).setMaxNumberOfSymbols(1));
	return !res.isEmpty() ? res.takeFirst() : Barcode();
}
#endif // QT_MULTIMEDIA_LIB

#define ZQ_PROPERTY(Type, name, setter) \
public: \
	Q_PROPERTY(Type name READ name WRITE setter NOTIFY name##Changed) \
	Type name() const noexcept { return ReaderOptions::name(); } \
	Q_SLOT void setter(const Type& newVal) \
	{ \
		if (name() != newVal) { \
			ReaderOptions::setter(newVal); \
			Q_EMIT name##Changed(); \
		} \
	} \
	Q_SIGNAL void name##Changed();


// MARK: - BarcodeReader

class BarcodeReader : public QObject, private ReaderOptions
{
	Q_OBJECT

	mutable QThreadPool _pool;

	Q_PROPERTY(BarcodeFormats formats READ formats WRITE setFormats NOTIFY formatsChanged)
	Q_PROPERTY(TextMode textMode READ textMode WRITE setTextMode NOTIFY textModeChanged)

	void emitFoundBarcodes(const QList<Barcode>& barcodes) const
	{
		if (!barcodes.isEmpty())
			Q_EMIT foundBarcodes(barcodes);
		else
			Q_EMIT foundNoBarcodes();
	}

public:
	BarcodeReader(QObject* parent = nullptr) : QObject(parent)
	{
		_pool.setMaxThreadCount(1);
	}

	~BarcodeReader()
	{
		_pool.setMaxThreadCount(0);
		_pool.waitForDone(-1);
	}

	Q_PROPERTY(int maxThreadCount READ maxThreadCount WRITE setMaxThreadCount)
	int maxThreadCount() const { return _pool.maxThreadCount(); }
	Q_SLOT void setMaxThreadCount(int maxThreadCount)
	{
		if (_pool.maxThreadCount() != maxThreadCount) {
			_pool.setMaxThreadCount(maxThreadCount);
			Q_EMIT maxThreadCountChanged();
		}
	}
	Q_SIGNAL void maxThreadCountChanged();

	BarcodeFormats formats() const noexcept { return Detail::transcode<BarcodeFormats>(ReaderOptions::formats()); }
	Q_SLOT void setFormats(const BarcodeFormats& newVal)
	{
		if (formats() != newVal) {
			ReaderOptions::setFormats(Detail::transcode<std::vector<ZXing::BarcodeFormat>>(newVal));
			Q_EMIT formatsChanged();
		}
	}
	// Q_SLOT void setFormats(BarcodeFormat newVal) { setFormats(BarcodeFormats({newVal})); }
	Q_SIGNAL void formatsChanged();

	TextMode textMode() const noexcept { return static_cast<TextMode>(ReaderOptions::textMode()); }
	Q_SLOT void setTextMode(TextMode newVal)
	{
		if (textMode() != newVal) {
			ReaderOptions::setTextMode(static_cast<ZXing::TextMode>(newVal));
			Q_EMIT textModeChanged();
		}
	}
	Q_SIGNAL void textModeChanged();

	// ZQ_PROPERTY(BarcodeFormats, formats, setFormats)
	// ZQ_PROPERTY(TextMode, textMode, setTextMode)
	ZQ_PROPERTY(bool, tryRotate, setTryRotate)
	ZQ_PROPERTY(bool, tryHarder, setTryHarder)
	ZQ_PROPERTY(bool, tryInvert, setTryInvert)
	ZQ_PROPERTY(bool, tryDownscale, setTryDownscale)
	ZQ_PROPERTY(bool, isPure, setIsPure)
	ZQ_PROPERTY(bool, returnErrors, setReturnErrors)

	// For debugging/development
	mutable QAtomicInt runTime = 0;
	Q_PROPERTY(int runTime MEMBER runTime)

	Q_SLOT QList<Barcode> read(const QImage& image) const
	{
		auto barcodes = ReadBarcodes(image, *this);
		emitFoundBarcodes(barcodes);
		return barcodes;
	}

	/// @brief  Read barcodes from the given image asynchronously.
	/// @note   The foundBarcodes() and foundNoBarcodes() signals are emitted from a worker thread.
	Q_SLOT void readAsync(const QImage& image) const
	{
		_pool.start([this, image]() { read(image); });
	}

Q_SIGNALS:
	/// @note If an async read is called, the foundBarcodes() and foundNoBarcodes() signals are emitted from a worker thread.
	void foundNoBarcodes() const;
	void foundBarcodes(const QList<Barcode>& barcodes) const;

public:
#ifdef QT_MULTIMEDIA_LIB
	Q_SLOT QList<Barcode> read(const QVideoFrame& image) const
	{
		QElapsedTimer t;
		t.start();
		auto barcodes = ReadBarcodes(image, *this);
		runTime = t.elapsed();
		emitFoundBarcodes(barcodes);
		return barcodes;
	}

	/// @brief  Try to read barcodes from the given video frame asynchronously.
	/// @return true iff a read task was started, false if all worker threads are busy.
	Q_SLOT bool tryReadAsync(const QVideoFrame& frame) const
	{
		return _pool.tryStart([this, frame]() { read(frame); });
	}

private:
	QVideoSink *_sink = nullptr;

public:
	void setVideoSink(QVideoSink* sink)
	{
		if (_sink == sink)
			return;

		if (_sink)
			disconnect(_sink, nullptr, this, nullptr);

		_sink = sink;
		if (_sink)
			connect(_sink, &QVideoSink::videoFrameChanged, this, &BarcodeReader::tryReadAsync, Qt::DirectConnection);
	}
	Q_PROPERTY(QVideoSink* videoSink MEMBER _sink WRITE setVideoSink)

#endif // QT_MULTIMEDIA_LIB

};

#undef ZQ_PROPERTY

} // namespace ZXingQt


// Q_DECLARE_METATYPE: compile-time declaration required for QVariant storage and template instantiation
Q_DECLARE_METATYPE(ZXingQt::Error)
Q_DECLARE_METATYPE(ZXingQt::Position)
Q_DECLARE_METATYPE(ZXingQt::Barcode)

#ifdef QT_QML_LIB

// MARK: - QML Integration

#include <QQmlEngine>

namespace ZXingQt {

class ZXingQml : public QObject
{
	Q_OBJECT
public:
	Q_INVOKABLE static QString FormatToString(BarcodeFormat format) { return ZXingQt::ToString(format); }
	Q_INVOKABLE static QString ContentTypeToString(ContentType type) { return ZXingQt::ToString(type); }
	Q_INVOKABLE static QList<BarcodeFormat> ListBarcodeFormats(BarcodeFormat filter = BarcodeFormat::None)
	{
		return ZXingQt::ListBarcodeFormats(filter);
	}
};

namespace Detail {

inline void registerQmlAndMetaTypes()
{
	// qRegisterMetaType: runtime registration required for queued signal/slot connections across threads
	qRegisterMetaType<ZXingQt::BarcodeFormat>("BarcodeFormat");
	qRegisterMetaType<ZXingQt::ContentType>("ContentType");
	qRegisterMetaType<ZXingQt::TextMode>("TextMode");
	qRegisterMetaType<ZXingQt::Error>("Error");
	qRegisterMetaType<ZXingQt::Error::Type>("Error::Type");
	qRegisterMetaType<ZXingQt::Position>("Position");
	qRegisterMetaType<ZXingQt::Barcode>("Barcode");
	qRegisterMetaType<QList<ZXingQt::BarcodeFormat>>("QList<BarcodeFormat>");
	qRegisterMetaType<QList<ZXingQt::Barcode>>("QList<Barcode>");

	// Custom enum to string converters
	QMetaType::registerConverter<BarcodeFormat, QString>(
		[](BarcodeFormat format) { return QString::fromStdString(ZXing::ToString(static_cast<ZXing::BarcodeFormat>(format))); });

	QMetaType::registerConverter<ContentType, QString>(
		[](ContentType type) { return QString::fromStdString(ZXing::ToString(static_cast<ZXing::ContentType>(type))); });

	// qmlRegisterType allows us to store the position / barcode in a QML property, i.e. property barcode myBarcode: ...
	qmlRegisterType<ZXingQt::Error>("ZXing", 1, 0, "error");
	qmlRegisterType<ZXingQt::Position>("ZXing", 1, 0, "position");
	qmlRegisterType<ZXingQt::Barcode>("ZXing", 1, 0, "barcode");

	qmlRegisterUncreatableMetaObject(ZXingQt::staticMetaObject, "ZXing", 1, 0, "ZXing", QStringLiteral("Access to enums only"));
	qmlRegisterType<ZXingQt::BarcodeReader>("ZXing", 1, 0, "BarcodeReader");

	qmlRegisterSingletonType<ZXingQml>("ZXing", 1, 0, "ZXingQml", [](QQmlEngine*, QJSEngine*) -> QObject* { return new ZXingQml(); });
}

struct ZXingQtInitializer
{
	ZXingQtInitializer() { registerQmlAndMetaTypes(); }
} inline zxingQtInitializer;

} // namespace Detail

} // namespace ZXingQt

#endif // QT_QML_LIB
