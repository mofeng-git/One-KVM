/*****************************************************************************
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
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


export function Keyboard() {
	var self = this;

	/************************************************************************/

	var __ws = null;
	var __online = true;

	var __keypad = null;
	var __use_release_hook = false;

	var __init__ = function() {
		__keypad = new Keypad("div#keyboard-window", __sendKey);

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

		if (tools.browser.is_mac) {
			// https://bugs.chromium.org/p/chromium/issues/detail?id=28089
			// https://bugzilla.mozilla.org/show_bug.cgi?id=1299553
			tools.info("Keyboard: enabled Mac-CMD-Hook");
			__use_release_hook = true;
		}
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		if (ws !== __ws) {
			self.releaseAll();
			__ws = ws;
		}
		__updateOnlineLeds();
	};

	self.setState = function(state) {
		__online = state.online;
		__updateOnlineLeds();

		for (let el of $$$(".hid-keyboard-leds")) {
			console.log(el, state.features.leds);
			el.classList.toggle("feature-disabled", !state.features.leds);
		}

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
		__keypad.releaseAll(__use_release_hook);
	};

	self.emit = function(code, state) {
		__keyboardHandler({code: code}, state);
	};

	var __updateOnlineLeds = function() {
		let is_captured = (
			$("stream-window").classList.contains("window-active")
			|| $("keyboard-window").classList.contains("window-active")
		);
		let led = "led-gray";
		let title = "Keyboard free";

		if (__ws) {
			if (__online) {
				if (is_captured) {
					led = "led-green";
					title = "Keyboard captured";
				}
			} else {
				led = "led-yellow";
				title = (is_captured ? "Keyboard captured, HID offline" : "Keyboard free, HID offline");
			}
		} else {
			if (is_captured) {
				title = "Keyboard captured, Pi-KVM offline";
			}
		}
		$("hid-keyboard-led").className = led;
		$("hid-keyboard-led").title = title;
	};

	var __keyboardHandler = function(event, state) {
		if (event.preventDefault) {
			event.preventDefault();
		}
		if (!event.repeat) {
			// https://bugs.chromium.org/p/chromium/issues/detail?id=28089
			// https://bugzilla.mozilla.org/show_bug.cgi?id=1299553
			__keypad.emit(event.code, state, __use_release_hook);
		}
	};

	var __sendKey = function(code, state) {
		tools.debug("Keyboard: key", (state ? "pressed:" : "released:"), code);
		if (__ws) {
			__ws.send(JSON.stringify({
				"event_type": "key",
				"event": {"key": code, "state": state},
			}));
		}
	};

	__init__();
}
