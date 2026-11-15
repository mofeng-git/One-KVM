cmake_minimum_required(VERSION 3.16)

project (ZXing VERSION "3.0.2")
set (ZXING_VERSION_SUFFIX "")
set (ZXING_SONAME 4) # see https://github.com/zxing-cpp/zxing-cpp/issues/333

foreach(format 1D AZTEC DATAMATRIX MAXICODE PDF417 QRCODE)
    option ("ZXING_ENABLE_${format}" "Enable support for ${format} barcodes" ON)
endforeach()

# One-KVM vendors only the QR reader. See ../README.md.
if (NOT ZXING_READERS OR ZXING_WRITERS OR ZXING_C_API OR
    ZXING_ENABLE_1D OR ZXING_ENABLE_AZTEC OR ZXING_ENABLE_DATAMATRIX OR
    ZXING_ENABLE_MAXICODE OR ZXING_ENABLE_PDF417 OR NOT ZXING_ENABLE_QRCODE)
    message(FATAL_ERROR "The vendored ZXing subset supports QR readers only")
endif()

if (NOT DEFINED CMAKE_CXX_STANDARD)
    set (CMAKE_CXX_STANDARD 20)
    set (CMAKE_CXX_STANDARD_REQUIRED ON)
    set (CMAKE_CXX_EXTENSIONS OFF)
endif()

if (BUILD_SHARED_LIBS)
    set(CMAKE_POSITION_INDEPENDENT_CODE ON)
endif()

if (NOT DEFINED ZXING_WRITERS)
    set (ZXING_WRITERS OFF)
endif()

if (NOT DEFINED ZXING_READERS)
    set (ZXING_READERS ON)
endif()

set (ZXING_WRITERS_NEW OFF)
set (ZXING_WRITERS_OLD OFF)
if (ZXING_WRITERS MATCHES "OLD")
    set (ZXING_WRITERS ON)
    set (ZXING_WRITERS_OLD ON)
elseif (ZXING_WRITERS MATCHES "NEW|ON")
    set (ZXING_WRITERS ON)
    set (ZXING_WRITERS_NEW ON)
elseif (ZXING_WRITERS MATCHES "BOTH")
    set (ZXING_WRITERS ON)
    set (ZXING_WRITERS_NEW ON)
    set (ZXING_WRITERS_OLD ON)
endif()

if (BUILD_SHARED_LIBS)
    set (CMAKE_WINDOWS_EXPORT_ALL_SYMBOLS ON)
endif()

set (ZXING_USE_ZINT ${ZXING_WRITERS_NEW}) # see Version.h.in

set (ZXING_PRIVATE_FLAGS
    -DZXING_INTERNAL
    $<$<BOOL:${ZXING_UNIT_TESTS}>:-DZXING_BUILD_FOR_TEST>
)

if (WINRT)
    set (ZXING_PUBLIC_FLAGS -DWINRT )
endif()

if (MSVC)
    list (APPEND ZXING_PRIVATE_FLAGS
        -D_SCL_SECURE_NO_WARNINGS
        -D_CRT_SECURE_NO_WARNINGS
        -D_CRT_NONSTDC_NO_WARNINGS
        -DNOMINMAX
        /utf-8 # see https://github.com/zxing-cpp/zxing-cpp/issues/757
    )
else()
    list (APPEND ZXING_PRIVATE_FLAGS
        -Wall -Wextra -Wno-missing-braces -Werror=undef -Werror=return-type)
endif()

include (CheckCXXCompilerFlag)

# This is needed for reproducible builds across different build directories.
# Without this, the usage of the __FILE__ macro leaves the build directory in
# the binary. When building the Python extension with build isolation enabled
# this would lead to random paths in the binary.
set(FILE_PREFIX_ARG "-fmacro-prefix-map=${CMAKE_CURRENT_SOURCE_DIR}/=")
check_cxx_compiler_flag("${FILE_PREFIX_ARG}" HAS_FILE_PREFIX_ARG)
if(HAS_FILE_PREFIX_ARG)
    list (APPEND ZXING_PRIVATE_FLAGS "${FILE_PREFIX_ARG}")
endif()

# MARK: - COMMON_FILES

set (COMMON_FILES
    src/Barcode.h
    src/Barcode.cpp
    src/BarcodeData.h
    src/BarcodeFormat.h
    src/BarcodeFormat.cpp
    src/BitMatrix.h
    src/BitMatrix.cpp
    src/BitMatrixIO.h
    src/BitMatrixIO.cpp
    src/ByteArray.h
    src/Content.h
    src/DecoderResult.h
    src/DetectorResult.h
    src/Error.h
    src/Error.cpp
    src/Flags.h
    src/ImageView.h
    src/JSON.h
    src/JSON.cpp
    src/Matrix.h
    src/Point.h
    src/Quadrilateral.h
    src/Range.h
    src/ReaderOptions.h
    src/ReadBarcode.h
    src/ReadBarcode.cpp
    src/SymbologyIdentifier.h
    src/ZXAlgorithms.h
    src/ZXConfig.h
    src/ZXTestSupport.h
)
if (ZXING_READERS OR ZXING_WRITERS_OLD)
    set (COMMON_FILES ${COMMON_FILES}
        src/BitArray.h
        src/BitArray.cpp
        src/GenericGF.h
        src/GenericGF.cpp
        src/GenericGFPoly.h
        src/GenericGFPoly.cpp
    )
endif()
if (ZXING_READERS)
    set (COMMON_FILES ${COMMON_FILES}
        src/BinaryBitmap.h
        src/BinaryBitmap.cpp
        src/BitMatrixCursor.h
        src/BitSource.h
        src/BitSource.cpp
        src/ConcentricFinder.h
        src/ConcentricFinder.cpp
        src/GlobalHistogramBinarizer.h
        src/GlobalHistogramBinarizer.cpp
        src/GridSampler.h
        src/GridSampler.cpp
        src/LogMatrix.h
        src/HybridBinarizer.h
        src/HybridBinarizer.cpp
        src/MultiFormatReader.h
        src/MultiFormatReader.cpp
        src/Pattern.h
        src/PerspectiveTransform.h
        src/PerspectiveTransform.cpp
        src/Reader.h
        src/ReedSolomonDecoder.h
        src/ReedSolomonDecoder.cpp
        src/RegressionLine.h
        src/ResultPoint.h
        src/ResultPoint.cpp
        src/StructuredAppend.h
        src/StdGenerator.h
        src/StdPrint.h
        src/StdScope.h
        src/ThresholdBinarizer.h
        src/TritMatrix.h # QRCode
    )
endif()

# MARK: - Precompiled reader headers

set (READER_HEADERS
    src/Barcode.h
    src/BarcodeFormat.h
    src/Error.h
    src/ImageView.h
    src/Point.h
    src/Quadrilateral.h
    src/ReadBarcode.h
    src/ReaderOptions.h
)

# MARK: - QR reader

if ((ZXING_READERS OR ZXING_WRITERS_OLD) AND ZXING_ENABLE_QRCODE)
    set (QRCODE_FILES
        src/qrcode/QRCodecMode.h
        src/qrcode/QRCodecMode.cpp
        src/qrcode/QRErrorCorrectionLevel.h
        src/qrcode/QRErrorCorrectionLevel.cpp
        src/qrcode/QRVersion.h
        src/qrcode/QRVersion.cpp
    )
endif()
if (ZXING_READERS AND ZXING_ENABLE_QRCODE)
    set (QRCODE_FILES ${QRCODE_FILES}
        src/qrcode/QRBitMatrixParser.h
        src/qrcode/QRBitMatrixParser.cpp
        src/qrcode/QRDataBlock.h
        src/qrcode/QRDataBlock.cpp
        src/qrcode/QRDataMask.h
        src/qrcode/QRDecoder.h
        src/qrcode/QRDecoder.cpp
        src/qrcode/QRDetector.h
        src/qrcode/QRDetector.cpp
        src/qrcode/QRECB.h
        src/qrcode/QRFormatInformation.h
        src/qrcode/QRFormatInformation.cpp
        src/qrcode/QRReader.h
        src/qrcode/QRReader.cpp
    )
endif()

source_group (Sources FILES ${COMMON_FILES})
source_group (Sources\\qrcode FILES ${QRCODE_FILES})

# MARK: - Library target

set(CMAKE_THREAD_PREFER_PTHREAD TRUE)
set(THREADS_PREFER_PTHREAD_FLAG TRUE)
find_package(Threads REQUIRED)

add_library (ZXing
    ${COMMON_FILES}
    ${QRCODE_FILES}
)

target_include_directories (ZXing
    PUBLIC "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>" "$<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}>"
)

target_compile_options (ZXing
    PUBLIC ${ZXING_PUBLIC_FLAGS}
    PRIVATE ${ZXING_PRIVATE_FLAGS}
)

target_compile_features(ZXing PUBLIC cxx_std_17)

target_link_libraries (ZXing PRIVATE Threads::Threads)

add_library(ZXing::ZXing ALIAS ZXing)
# force position independent code to be able to link it as static lib into a DLL (e.g. the python module)
set_target_properties(ZXing PROPERTIES POSITION_INDEPENDENT_CODE ON)
if (PROJECT_VERSION)
    set_target_properties(ZXing PROPERTIES VERSION ${PROJECT_VERSION})
    set_target_properties(ZXing PROPERTIES SOVERSION ${ZXING_SONAME})
endif()

# MARK: - Precompiled headers

set(PRECOMPILE_HEADERS ${READER_HEADERS})
list(APPEND PRECOMPILE_HEADERS "src/BarcodeData.h" "src/Content.h" "src/Range.h")
if (NOT ZXING_DISABLE_PCH)
    target_precompile_headers(ZXing PRIVATE ${PRECOMPILE_HEADERS})
endif()

if(CMAKE_BUILD_TYPE MATCHES "Release|RelWithDebInfo")
    # The following is a list of translation units that fulfill two criteria regarding the use of -Os vs -O3:
    #  1. their binary size decreases significantly
    #  2. the runtime of ReaderTest is not (measurably) affected
    # Compiling them with -Os saves about 40kB (3%) with clang and 190kB (12%) with gcc.
    check_cxx_compiler_flag("-Os" COMPILER_KNOWS_Os)
    if(COMPILER_KNOWS_Os)
    set_source_files_properties(
        src/Barcode.cpp
        src/BarcodeFormat.cpp
        src/BitMatrixIO.cpp
        src/Error.cpp
        src/MultiFormatReader.cpp
        src/qrcode/QRBitMatrixParser.cpp
        src/qrcode/QRDataBlock.cpp
        src/qrcode/QRDecoder.cpp
        src/qrcode/QRReader.cpp
        src/qrcode/QRVersion.cpp
        PROPERTIES SKIP_PRECOMPILE_HEADERS ON COMPILE_FLAGS -Os)
    endif()
endif()

# Version.h supplies feature macros used by the QR reader. Static archives are
# installed by the parent project for Cargo; no standalone package is exported.
configure_file (Version.h.in Version.h)
