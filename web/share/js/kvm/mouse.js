/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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


"use strict";


import {tools, $} from "../tools.js";
import {Keypad} from "../keypad.js";


export function Mouse(__getResolution, __recordWsEvent) {
	var self = this;

	/************************************************************************/

	var __ws = null;
	var __online = true;
	var __absolute = true;

	var __keypad = null;

	var __timer = null;
	var __current_pos = {x: 0, y: 0};
	var __sent_pos = {x: 0, y: 0};
	var __wheel_delta = {x: 0, y: 0};
	var __relative_deltas = [];
	var __relative_sens = 1.0;

	var __stream_hovered = false;

	var __init__ = function() {
		__keypad = new Keypad("div#stream-mouse-buttons", __sendButton);

		$("hid-mouse-led").title = "Mouse free";

		document.onpointerlockchange = __relativeCapturedHandler; // Only for relative
		document.onpointerlockerror = __relativeCapturedHandler;
		$("stream-box").onmouseenter = () => __streamHoveredHandler(true);
		$("stream-box").onmouseleave = () => __streamHoveredHandler(false);
		$("stream-box").onmousedown = (event) => __streamButtonHandler(event, true);
		$("stream-box").onmouseup = (event) => __streamButtonHandler(event, false);
		$("stream-box").oncontextmenu = (event) => event.preventDefault();
		$("stream-box").onmousemove = __streamMoveHandler;
		$("stream-box").onwheel = __streamWheelHandler;
		$("stream-box").ontouchstart = (event) => __streamTouchMoveHandler(event);

		tools.storage.bindSimpleSwitch($("hid-mouse-squash-switch"), "hid.mouse.squash", true);
		tools.slider.setParams($("hid-mouse-sens-slider"), 0.1, 1.9, 0.1, tools.storage.get("hid.mouse.sens", 1.0), __updateRelativeSens);
		tools.slider.setParams($("hid-mouse-rate-slider"), 10, 100, 10, tools.storage.get("hid.mouse.rate", 100), __updateRate); // set __timer
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		__ws = ws;
		$("stream-box").classList.toggle("stream-box-mouse-enabled", ws);
		if (!__absolute && __isRelativeCaptured()) {
			document.exitPointerLock();
		}
		__updateOnlineLeds();
	};

	self.setState = function(state, hid_online, hid_busy) {
		if (!hid_online) {
			__online = null;
		} else {
			__online = (state.online && !hid_busy);
		}
		if (!__absolute && state.absolute && __isRelativeCaptured()) {
			document.exitPointerLock();
		}
		if (__absolute && !state.absolute) {
			__relative_deltas = [];
		}
		__absolute = state.absolute;
		__updateOnlineLeds();
	};

	self.releaseAll = function() {
		__keypad.releaseAll();
	};

	var __updateRate = function(value) {
		$("hid-mouse-rate-value").innerHTML = value;
		tools.storage.set("hid.mouse.rate", value);
		if (__timer) {
			clearInterval(__timer);
		}
		__timer = setInterval(__sendMove, value);
	};

	var __updateRelativeSens = function(value) {
		$("hid-mouse-sens-value").innerHTML = value.toFixed(1);
		tools.storage.set("hid.mouse.sens", value);
		__relative_sens = value;
	};

	var __streamHoveredHandler = function(hovered) {
		if (__absolute) {
			__stream_hovered = hovered;
			__updateOnlineLeds();
		}
	};

	var __updateOnlineLeds = function() {
		let is_captured;
		if (__absolute) {
			is_captured = (__stream_hovered || tools.browser.is_ios);
		} else {
			is_captured = __isRelativeCaptured();
		}
		let led = "led-gray";
		let title = "Mouse free";

		if (__ws) {
			if (__online === null) {
				led = "led-red";
				title = (is_captured ? "Mouse captured, HID offline" : "Mouse free, HID offline");
			} else if (__online) {
				if (is_captured) {
					led = "led-green";
					title = "Mouse captured";
				}
			} else {
				led = "led-yellow";
				title = (is_captured ? "Mouse captured, inactive/busy" : "Mouse free, inactive/busy");
			}
		} else {
			if (is_captured) {
				title = "Mouse captured, PiKVM offline";
			}
		}
		$("hid-mouse-led").className = led;
		$("hid-mouse-led").title = title;
	};

	var __isRelativeCaptured = function() {
		return (document.pointerLockElement === $("stream-box"));
	};

	var __isRelativeSquashed = function() {
		return $("hid-mouse-squash-switch").checked;
	};

	var __relativeCapturedHandler = function() {
		tools.info("Relative mouse", (__isRelativeCaptured() ? "captured" : "released"), "by pointer lock");
		__updateOnlineLeds();
	};

	var __streamButtonHandler = function(event, state) {
		// https://www.w3schools.com/jsref/event_button.asp
		event.preventDefault();
		if (__absolute || __isRelativeCaptured()) {
			switch (event.button) {
				case 0: __keypad.emit("left", state); break;
				case 2: __keypad.emit("right", state); break;
				case 1: __keypad.emit("middle", state); break;
				case 3: __keypad.emit("up", state); break;
				case 4: __keypad.emit("down", state); break;
			}
		} else if (!__absolute && !__isRelativeCaptured() && !state) {
			$("stream-box").requestPointerLock();
		}
	};

	var __streamTouchMoveHandler = function(event) {
		event.preventDefault();
		if (__absolute) {
			if (event.touches[0].target && event.touches[0].target.getBoundingClientRect) {
				let rect = event.touches[0].target.getBoundingClientRect();
				__current_pos = {
					x: Math.round(event.touches[0].clientX - rect.left),
					y: Math.round(event.touches[0].clientY - rect.top),
				};
				__sendMove();
			}
		}
	};

	var __streamMoveHandler = function(event) {
		if (__absolute) {
			let rect = event.target.getBoundingClientRect();
			__current_pos = {
				x: Math.max(Math.round(event.clientX - rect.left), 0),
				y: Math.max(Math.round(event.clientY - rect.top), 0),
			};
		} else if (__isRelativeCaptured()) {
			let delta = {
				x: Math.min(Math.max(-127, Math.floor(event.movementX * __relative_sens)), 127),
				y: Math.min(Math.max(-127, Math.floor(event.movementY * __relative_sens)), 127),
			};
			if (__isRelativeSquashed()) {
				__relative_deltas.push(delta);
			} else {
				tools.debug("Mouse: relative:", delta);
				__sendEvent("mouse_relative", {"delta": delta});
			}
		}
	};

	var __sendButton = function(button, state) {
		tools.debug("Mouse: button", (state ? "pressed:" : "released:"), button);
		__sendMove();
		__sendEvent("mouse_button", {"button": button, "state": state});
	};

	var __sendMove = function() {
		if (__absolute) {
			let pos = __current_pos;
			if (pos.x !== __sent_pos.x || pos.y !== __sent_pos.y) {
				let geo = __getVideoGeometry();
				let to = {
					"x": __translatePosition(pos.x, geo.x, geo.width, -32768, 32767),
					"y": __translatePosition(pos.y, geo.y, geo.height, -32768, 32767),
				};
				tools.debug("Mouse: moved:", to);
				__sendEvent("mouse_move", {"to": to});
				__sent_pos = pos;
			}
		} else if (__relative_deltas.length) {
			tools.debug("Mouse: relative:", __relative_deltas);
			__sendEvent("mouse_relative", {"delta": __relative_deltas, "squash": true});
			__relative_deltas = [];
		}
	};

	var __getVideoGeometry = function() {
		// Первоначально обновление геометрии считалось через ResizeObserver.
		// Но оно не ловило некоторые события, например в последовательности:
		//   - Находять в HD переходим в фулскрин
		//   - Меняем разрешение на маленькое
		//   - Убираем фулскрин
		//   - Переходим в HD
		//   - Видим нарушение пропорций
		// Так что теперь используются быстре рассчеты через offset*
		// вместо getBoundingClientRect().
		let res = __getResolution();
		let ratio = Math.min(res.view_width / res.real_width, res.view_height / res.real_height);
		return {
			"x": Math.round((res.view_width - ratio * res.real_width) / 2),
			"y": Math.round((res.view_height - ratio * res.real_height) / 2),
			"width": Math.round(ratio * res.real_width),
			"height": Math.round(ratio * res.real_height),
		};
	};

	var __translatePosition = function(x, a, b, c, d) {
		let translated = Math.round((x - a) / b * (d - c) + c);
		if (translated < c) {
			return c;
		} else if (translated > d) {
			return d;
		}
		return translated;
	};

	var __streamWheelHandler = function(event) {
		// https://learn.javascript.ru/mousewheel
		// https://stackoverflow.com/a/24595588
		if (event.preventDefault) {
			event.preventDefault();
		}

		if (!__absolute && !__isRelativeCaptured()) {
			return;
		}

		let delta = {x: 0, y: 0};

		if (tools.browser.is_firefox && !tools.browser.is_mac) {
			if (event.deltaX !== 0) {
				delta.x = event.deltaX / Math.abs(event.deltaX) * (-5);
			}
			if (event.deltaY !== 0) {
				delta.y = event.deltaY / Math.abs(event.deltaY) * (-5);
			}
		} else {
			let factor = (tools.browser.is_mac ? 5 : 1);

			__wheel_delta.x += event.deltaX * factor; // Horizontal scrolling
			if (Math.abs(__wheel_delta.x) >= 100) {
				delta.x = __wheel_delta.x / Math.abs(__wheel_delta.x) * (-5);
				__wheel_delta.x = 0;
			}

			__wheel_delta.y += event.deltaY * factor; // Vertical scrolling
			if (Math.abs(__wheel_delta.y) >= 100) {
				delta.y = __wheel_delta.y / Math.abs(__wheel_delta.y) * (-5);
				__wheel_delta.y = 0;
			}
		}

		if (delta.x || delta.y) {
			tools.debug("Mouse: scrolled:", delta);
			__sendEvent("mouse_wheel", {"delta": delta});
		}
	};

	var __sendEvent = function(event_type, event) {
		event = {"event_type": event_type, "event": event};
		if (__ws && !$("hid-mute-switch").checked) {
			__ws.send(JSON.stringify(event));
		}
		__recordWsEvent(event);
	};

	__init__();
}
