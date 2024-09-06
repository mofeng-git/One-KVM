/* ========================================================================= #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
#                                                                            #
#    This program is free software: you can redistribute it and/or modify    #
#    it under the terms of the GNU General Public License as published by    #
#    the Free Software Foundation, either version 3 of the License, or       #
#    (at your option) any later version.                                     #
#                                                                            #
#    This program is distributed in the hope that it will be useful,         #
#    but WITHOUT ANY WARRANTY; without even the implied warranty of          #
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the           #
#    GNU General Public License for more details.                            #
#                                                                            #
#    You should have received a copy of the GNU General Public License       #
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.  #
#                                                                            #
# ========================================================================= */


#pragma once


//--------------------------------------------------------------------
// Common config
//--------------------------------------------------------------------

//#define CFG_TUSB_DEBUG 100

#define CFG_TUSB_OS OPT_OS_PICO

// Enable device stack
#define CFG_TUD_ENABLED 1

// CFG_TUSB_DEBUG is defined by compiler in DEBUG build
//#define CFG_TUSB_DEBUG 100

// USB DMA on some MCUs can only access a specific SRAM region with restriction on alignment.
// Tinyusb use follows macros to declare transferring memory so that they can be put
// into those specific section.
//   - CFG_TUSB_MEM SECTION : __attribute__ (( section(".usb_ram") ))
//   - CFG_TUSB_MEM_ALIGN   : __attribute__ ((aligned(4)))
#ifndef CFG_TUSB_MEM_SECTION
#	define CFG_TUSB_MEM_SECTION
#endif

#ifndef CFG_TUSB_MEM_ALIGN
#	define CFG_TUSB_MEM_ALIGN __attribute__((aligned(4)))
#endif


//--------------------------------------------------------------------
// Device config
//--------------------------------------------------------------------

#ifndef CFG_TUD_ENDPOINT0_SIZE
#	define CFG_TUD_ENDPOINT0_SIZE 64
#endif

// HID: Keyboard + Mouse
#define CFG_TUD_HID 2

// HID buffer size Should be sufficient to hold ID (if any) + Data
#ifndef CFG_TUD_HID_EP_BUFSIZE
#	define CFG_TUD_HID_EP_BUFSIZE 16
#endif


// CDC for the bridge mode
#define CFG_TUD_CDC             1

// CDC FIFO size of TX and RX
#define CFG_TUD_CDC_RX_BUFSIZE 4096
#define CFG_TUD_CDC_TX_BUFSIZE 4096

// CDC Endpoint transfer buffer size, more is faster
#define CFG_TUD_CDC_EP_BUFSIZE 64
