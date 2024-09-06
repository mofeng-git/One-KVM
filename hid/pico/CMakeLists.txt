cmake_minimum_required(VERSION 3.13)

set(PICO_SDK_PATH ${CMAKE_CURRENT_LIST_DIR}/.pico-sdk)
set(PICO_TINYUSB_PATH ${CMAKE_CURRENT_LIST_DIR}/.tinyusb)
set(PS2_PATH ${CMAKE_CURRENT_LIST_DIR}/.ps2x2pico/src)

# For TinyUSB
set(FAMILY rp2040)

# Include pico_sdk_import.cmake from pico-sdk (instead of copying)
include(${PICO_SDK_PATH}/external/pico_sdk_import.cmake)

# Generic setup
set(PROJECT hid)
project(${PROJECT})

# Initialize Pico-SDK
pico_sdk_init()

# Set the path to the source code to build
set(SRC_TO_BUILD_PATH ${CMAKE_CURRENT_LIST_DIR}/src)
add_subdirectory(${SRC_TO_BUILD_PATH})
