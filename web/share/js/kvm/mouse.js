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


"use strict";


import {tools, $} from "../tools.js";
import {Keypad} from "../keypad.js";


export function Mouse(record_callback) {
	var self = this;

	/************************************************************************/

	var __record_callback = record_callback;

	var __ws = null;
	var __online = true;
	var __absolute = true;

	var __keypad = null;

	var __current_pos = {x: 0, y:0};
	var __sent_pos = {x: 0, y:0};
	var __wheel_delta = {x: 0, y: 0};
	var __relative_deltas = [];

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

		setInterval(__sendMove, 100);
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		__ws = ws;
		$("stream-box").classList.toggle("stream-box-mouse-enabled", ws);
		if (!__absolute && __isRelativeCaptured()) {
			$("stream-box").exitPointerLock();
		}
		__updateOnlineLeds();
	};

	self.setState = function(state) {
		__online = state.online;
		if (!("absolute" in state)) { // FIXME: SPI
			state.absolute = true;
		}
		if (!__absolute && state.absolute && __isRelativeCaptured()) {
			$("stream-box").exitPointerLock();
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

	var __streamHoveredHandler = function(hovered) {
		if (__absolute) {
			__stream_hovered = hovered;
			__updateOnlineLeds();
		}
	};

	var __updateOnlineLeds = function() {
		let captured;
		if (__absolute) {
			captured = (__stream_hovered || tools.browser.is_ios);
		} else {
			captured = __isRelativeCaptured();
		}
		let led = "led-gray";
		let title = "Mouse free";

		if (__ws) {
			if (__online) {
				if (captured) {
					led = "led-green";
					title = "Mouse captured";
				}
			} else {
				led = "led-yellow";
				title = (captured ? "Mouse captured, HID offline" : "Mouse free, HID offline");
			}
		} else {
			if (captured) {
				title = "Mouse captured, Pi-KVM offline";
			}
		}
		$("hid-mouse-led").className = led;
		$("hid-mouse-led").title = title;
	};

	var __isRelativeCaptured = function() {
		return (document.pointerLockElement === $("stream-box"));
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
				x: Math.min(Math.max(-127, event.movementX), 127),
				y: Math.min(Math.max(-127, event.movementY), 127),
			};
			__relative_deltas.push(delta);
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
				let el_stream_image = $("stream-image");
				let to = {
					x: __translate(pos.x, 0, el_stream_image.clientWidth, -32768, 32767),
					y: __translate(pos.y, 0, el_stream_image.clientHeight, -32768, 32767),
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

	var __translate = function(x, a, b, c, d) {
		return Math.round((x - a) / (b - a) * (d - c) + c);
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
		if (__ws) {
			__ws.send(JSON.stringify(event));
		}
		__record_callback(event);
	};

	__init__();
}
