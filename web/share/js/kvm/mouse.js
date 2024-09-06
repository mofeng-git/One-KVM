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


"use strict";


import {tools, $} from "../tools.js";
import {Keypad} from "../keypad.js";


export function Mouse(__getGeometry, __recordWsEvent) {
	var self = this;

	/************************************************************************/

	var __ws = null;
	var __online = true;
	var __absolute = true;

	var __keypad = null;

	var __timer = null;
	var __planned_pos = {"x": 0, "y": 0};
	var __sent_pos = {"x": 0, "y": 0};
	var __relative_deltas = [];
	var __relative_touch_pos = null;
	var __relative_sens = 1.0;
	var __scroll_rate = 5;
	var __scroll_delta = {"x": 0, "y": 0};

	var __stream_hovered = false;

	var __init__ = function() {
		__keypad = new Keypad("div#stream-mouse-buttons", __sendButton, false);

		$("hid-mouse-led").title = "Mouse free";

		document.onpointerlockchange = __relativeCapturedHandler; // Only for relative
		document.onpointerlockerror = __relativeCapturedHandler;
		$("stream-box").onmouseenter = () => __streamHoveredHandler(true);
		$("stream-box").onmouseleave = () => __streamHoveredHandler(false);
		$("stream-box").onmousedown = (event) => __streamButtonHandler(event, true);
		$("stream-box").onmouseup = (event) => __streamButtonHandler(event, false);
		$("stream-box").oncontextmenu = (event) => event.preventDefault();
		$("stream-box").onmousemove = __streamMoveHandler;
		$("stream-box").onwheel = __streamScrollHandler;
		$("stream-box").ontouchstart = (event) => __streamTouchStartHandler(event);
		$("stream-box").ontouchmove = (event) => __streamTouchMoveHandler(event);
		$("stream-box").ontouchend = (event) => __streamTouchEndHandler(event);

		tools.storage.bindSimpleSwitch($("hid-mouse-squash-switch"), "hid.mouse.squash", true);
		tools.slider.setParams($("hid-mouse-sens-slider"), 0.1, 1.9, 0.1, tools.storage.get("hid.mouse.sens", 1.0), __updateRelativeSens);
		tools.slider.setParams($("hid-mouse-rate-slider"), 10, 100, 10, tools.storage.get("hid.mouse.rate", 10), __updateRate); // set __timer

		tools.storage.bindSimpleSwitch($("hid-mouse-reverse-scrolling-switch"), "hid.mouse.reverse_scrolling", false);
		tools.storage.bindSimpleSwitch($("hid-mouse-reverse-panning-switch"), "hid.mouse.reverse_panning", false);
		let cumulative_scrolling = !(tools.browser.is_firefox && !tools.browser.is_mac);
		tools.storage.bindSimpleSwitch($("hid-mouse-cumulative-scrolling-switch"), "hid.mouse.cumulative_scrolling", cumulative_scrolling);
		tools.slider.setParams($("hid-mouse-scroll-slider"), 1, 25, 1, tools.storage.get("hid.mouse.scroll_rate", 5), __updateScrollRate);

		tools.storage.bindSimpleSwitch($("hid-mouse-dot-switch"), "hid.mouse.dot", true, __updateOnlineLeds);
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		__ws = ws;
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
			__relative_touch_pos = null;
		}
		__absolute = state.absolute;
		__updateOnlineLeds();
	};

	self.releaseAll = function() {
		__keypad.releaseAll();
	};

	var __updateRate = function(value) {
		$("hid-mouse-rate-value").innerHTML = value + " ms";
		tools.storage.set("hid.mouse.rate", value);
		if (__timer) {
			clearInterval(__timer);
		}
		__timer = setInterval(__sendPlannedMove, value);
	};

	var __updateScrollRate = function(value) {
		$("hid-mouse-scroll-value").innerHTML = value;
		tools.storage.set("hid.mouse.scroll_rate", value);
		__scroll_rate = value;
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
			is_captured = (__stream_hovered || tools.browser.is_mobile);
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

		if (__absolute && is_captured) {
			let dot = $("hid-mouse-dot-switch").checked;
			$("stream-box").classList.toggle("stream-box-mouse-dot", (dot && __ws));
			$("stream-box").classList.toggle("stream-box-mouse-none", (!dot && __ws));
		} else {
			$("stream-box").classList.toggle("stream-box-mouse-dot", false);
			$("stream-box").classList.toggle("stream-box-mouse-none", false);
		}
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
				case 0: __keypad.emitByCode("left", state); break;
				case 2: __keypad.emitByCode("right", state); break;
				case 1: __keypad.emitByCode("middle", state); break;
				case 3: __keypad.emitByCode("up", state); break;
				case 4: __keypad.emitByCode("down", state); break;
			}
		} else if (!__absolute && !__isRelativeCaptured() && !state) {
			$("stream-box").requestPointerLock();
		}
	};

	var __streamTouchStartHandler = function(event) {
		event.preventDefault();
		if (event.touches.length === 1) {
			if (__absolute) {
				__planned_pos = __getTouchPosition(event, 0);
				__sendPlannedMove();
			} else {
				__relative_touch_pos = __getTouchPosition(event, 0);
			}
		}
	};

	var __streamTouchMoveHandler = function(event) {
		event.preventDefault();
		if (event.touches.length === 1) {
			if (__absolute) {
				__planned_pos = __getTouchPosition(event, 0);
			} else if (__relative_touch_pos === null) {
				__relative_touch_pos = __getTouchPosition(event, 0);
			} else {
				let pos = __getTouchPosition(event, 0);
				__sendOrPlanRelativeMove({
					"x": (pos.x - __relative_touch_pos.x),
					"y": (pos.y - __relative_touch_pos.y),
				});
				__relative_touch_pos = pos;
			}
		}
	};

	var __streamTouchEndHandler = function(event) {
		event.preventDefault();
		__sendPlannedMove();
	};

	var __getTouchPosition = function(event, index) {
		if (event.touches[index].target && event.touches[index].target.getBoundingClientRect) {
			let rect = event.touches[index].target.getBoundingClientRect();
			return {
				"x": Math.round(event.touches[index].clientX - rect.left),
				"y": Math.round(event.touches[index].clientY - rect.top),
			};
		}
		return null;
	};

	var __streamMoveHandler = function(event) {
		if (__absolute) {
			let rect = event.target.getBoundingClientRect();
			__planned_pos = {
				"x": Math.max(Math.round(event.clientX - rect.left), 0),
				"y": Math.max(Math.round(event.clientY - rect.top), 0),
			};
		} else if (__isRelativeCaptured()) {
			__sendOrPlanRelativeMove({
				"x": event.movementX,
				"y": event.movementY,
			});
		}
	};

	var __streamScrollHandler = function(event) {
		// https://learn.javascript.ru/mousewheel
		// https://stackoverflow.com/a/24595588

		event.preventDefault();

		if (!__absolute && !__isRelativeCaptured()) {
			return;
		}

		let delta = {"x": 0, "y": 0};
		if ($("hid-mouse-cumulative-scrolling-switch").checked) {
			let factor = (tools.browser.is_mac ? 5 : 1);

			__scroll_delta.x += event.deltaX * factor; // Horizontal scrolling
			if (Math.abs(__scroll_delta.x) >= 100) {
				delta.x = __scroll_delta.x / Math.abs(__scroll_delta.x) * (-__scroll_rate);
				__scroll_delta.x = 0;
			}

			__scroll_delta.y += event.deltaY * factor; // Vertical scrolling
			if (Math.abs(__scroll_delta.y) >= 100) {
				delta.y = __scroll_delta.y / Math.abs(__scroll_delta.y) * (-__scroll_rate);
				__scroll_delta.y = 0;
			}
		} else {
			if (event.deltaX !== 0) {
				delta.x = event.deltaX / Math.abs(event.deltaX) * (-__scroll_rate);
			}
			if (event.deltaY !== 0) {
				delta.y = event.deltaY / Math.abs(event.deltaY) * (-__scroll_rate);
			}
		}

		__sendScroll(delta);
	};

	var __sendOrPlanRelativeMove = function(delta) {
		delta = {
			"x": Math.min(Math.max(-127, Math.floor(delta.x * __relative_sens)), 127),
			"y": Math.min(Math.max(-127, Math.floor(delta.y * __relative_sens)), 127),
		};
		if (delta.x || delta.y) {
			if ($("hid-mouse-squash-switch").checked) {
				__relative_deltas.push(delta);
			} else {
				tools.debug("Mouse: relative:", delta);
				__sendEvent("mouse_relative", {"delta": delta});
			}
		}
	};

	var __sendScroll = function(delta) {
		if (delta.x || delta.y) {
			if ($("hid-mouse-reverse-scrolling-switch").checked) {
				delta.y *= -1;
			}
			if ($("hid-mouse-reverse-panning-switch").checked) {
				delta.x *= -1;
			}
			tools.debug("Mouse: scrolled:", delta);
			__sendEvent("mouse_wheel", {"delta": delta});
		}
	};

	var __sendPlannedMove = function() {
		if (__absolute) {
			let pos = __planned_pos;
			if (pos.x !== __sent_pos.x || pos.y !== __sent_pos.y) {
				let geo = __getGeometry();
				let to = {
					"x": tools.remap(pos.x, geo.x, geo.width, -32768, 32767),
					"y": tools.remap(pos.y, geo.y, geo.height, -32768, 32767),
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

	var __sendButton = function(button, state) {
		tools.debug("Mouse: button", (state ? "pressed:" : "released:"), button);
		__sendPlannedMove();
		__sendEvent("mouse_button", {"button": button, "state": state});
	};

	var __sendEvent = function(event_type, event) {
		event = {"event_type": event_type, "event": event};
		if (__ws && !$("hid-mute-switch").checked) {
			__ws.sendHidEvent(event);
		}
		__recordWsEvent(event);
	};

	__init__();
}
