/*****************************************************************************
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
*****************************************************************************/


import {tools, $, $$$} from "../tools.js";
import {Keypad} from "../keypad.js";


export function Keyboard(__recordWsEvent) {
	var self = this;

	/************************************************************************/

	var __ws = null;
	var __online = true;

	var __keypad = null;

	var __init__ = function() {
		__keypad = new Keypad("div#keyboard-window", __sendKey, true);

		$("hid-keyboard-led").title = "Keyboard free";

		$("keyboard-window").onkeydown = (event) => __keyboardHandler(event, true);
		$("keyboard-window").onkeyup = (event) => __keyboardHandler(event, false);
		$("keyboard-window").onfocus = __updateOnlineLeds;
		$("keyboard-window").onblur = __updateOnlineLeds;

		$("stream-window").onkeydown = (event) => __keyboardHandler(event, true);
		$("stream-window").onkeyup = (event) => __keyboardHandler(event, false);
		$("stream-window").onfocus = __updateOnlineLeds;
		$("stream-window").onblur = __updateOnlineLeds;

		window.addEventListener("focusin", __updateOnlineLeds);
		window.addEventListener("focusout", __updateOnlineLeds);

		tools.storage.bindSimpleSwitch($("hid-keyboard-swap-cc-switch"), "hid.keyboard.swap_cc", false);
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		if (ws !== __ws) {
			self.releaseAll();
			__ws = ws;
		}
		__updateOnlineLeds();
	};

	self.setState = function(state, hid_online, hid_busy) {
		if (!hid_online) {
			__online = null;
		} else {
			__online = (state.online && !hid_busy);
		}
		__updateOnlineLeds();

		for (let led of ["caps", "scroll", "num"]) {
			for (let el of $$$(`.hid-keyboard-${led}-led`)) {
				if (state.leds[led]) {
					el.classList.add("led-green");
					el.classList.remove("led-gray");
				} else {
					el.classList.add("led-gray");
					el.classList.remove("led-green");
				}
			}
		}
	};

	self.releaseAll = function() {
		__keypad.releaseAll();
	};

	self.emit = function(code, state) {
		__keypad.emitByCode(code, state);
	};

	var __updateOnlineLeds = function() {
		let is_captured = (
			$("stream-window").classList.contains("window-active")
			|| $("keyboard-window").classList.contains("window-active")
		);
		let led = "led-gray";
		let title = "Keyboard free";

		if (__ws) {
			if (__online === null) {
				led = "led-red";
				title = (is_captured ? "Keyboard captured, HID offline" : "Keyboard free, HID offline");
			} else if (__online) {
				if (is_captured) {
					led = "led-green";
					title = "Keyboard captured";
				}
			} else {
				led = "led-yellow";
				title = (is_captured ? "Keyboard captured, inactive/busy" : "Keyboard free, inactive/busy");
			}
		} else {
			if (is_captured) {
				title = "Keyboard captured, PiKVM offline";
			}
		}
		$("hid-keyboard-led").className = led;
		$("hid-keyboard-led").title = title;
	};

	var __keyboardHandler = function(event, state) {
		event.preventDefault();
		__keypad.emitByKeyEvent(event, state);
	};

	var __sendKey = function(code, state) {
		tools.debug("Keyboard: key", (state ? "pressed:" : "released:"), code);
		if ($("hid-keyboard-swap-cc-switch").checked) {
			if (code === "ControlLeft") {
				code = "CapsLock";
			} else if (code === "CapsLock") {
				code = "ControlLeft";
			}
		}
		let event = {
			"event_type": "key",
			"event": {"key": code, "state": state},
		};
		if (__ws && !$("hid-mute-switch").checked) {
			__ws.sendHidEvent(event);
		}
		__recordWsEvent(event);
	};

	__init__();
}
