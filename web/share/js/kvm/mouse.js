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


function Mouse() {
	var self = this;

	/************************************************************************/

	var __ws = null;
	var __online = true;

	var __keypad = null;

	var __current_pos = {x: 0, y:0};
	var __sent_pos = {x: 0, y:0};
	var __wheel_delta = {x: 0, y: 0};

	var __stream_hovered = false;

	var __init__ = function() {
		__keypad = new Keypad("div#stream-mouse-buttons", __sendButton);

		$("hid-mouse-led").title = "Mouse free";

		$("stream-box").onmouseenter = __hoverStream;
		$("stream-box").onmouseleave = __leaveStream;
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
		if (ws) {
			$("stream-box").classList.add("stream-box-mouse-enabled");
		} else {
			$("stream-box").classList.remove("stream-box-mouse-enabled");
		}
		__updateLeds();
	};

	self.setState = function(state) {
		__online = state.online;
		__updateLeds();
	};

	self.releaseAll = function() {
		__keypad.releaseAll();
	};

	var __hoverStream = function() {
		__stream_hovered = true;
		__updateLeds();
	};

	var __leaveStream = function() {
		__stream_hovered = false;
		__updateLeds();
	};

	var __updateLeds = function() {
		let is_captured = (__stream_hovered || tools.browser.is_ios);
		let led = "led-gray";
		let title = "Mouse free";

		if (__ws) {
			if (__online) {
				if (is_captured) {
					led = "led-green";
					title = "Mouse captured";
				}
			} else {
				led = "led-yellow";
				title = (is_captured ? "Mouse captured, HID offline" : "Mouse free, HID offline");
			}
		} else {
			if (is_captured) {
				title = "Mouse captured, Pi-KVM offline";
			}
		}
		$("hid-mouse-led").className = led;
		$("hid-mouse-led").title = title;
	};

	var __streamButtonHandler = function(event, state) {
		// https://www.w3schools.com/jsref/event_button.asp
		event.preventDefault();
		switch (event.button) {
			case 0: __keypad.emit("left", state); break;
			case 2: __keypad.emit("right", state); break;
		}
	};

	var __streamTouchMoveHandler = function(event) {
		event.preventDefault();
		if (event.touches[0].target && event.touches[0].target.getBoundingClientRect) {
			let rect = event.touches[0].target.getBoundingClientRect();
			__current_pos = {
				x: Math.round(event.touches[0].clientX - rect.left),
				y: Math.round(event.touches[0].clientY - rect.top),
			};
			__sendMove();
		}
	};

	var __streamMoveHandler = function(event) {
		let rect = event.target.getBoundingClientRect();
		__current_pos = {
			x: Math.round(event.clientX - rect.left),
			y: Math.round(event.clientY - rect.top),
		};
	};


	var __sendButton = function(button, state) {
		tools.debug("Mouse: button", (state ? "pressed:" : "released:"), button);
		__sendMove();
		if (__ws) {
			__ws.send(JSON.stringify({
				event_type: "mouse_button",
				button: button,
				state: state,
			}));
		}
	};

	var __sendMove = function() {
		let pos = __current_pos;
		if (pos.x !== __sent_pos.x || pos.y !== __sent_pos.y) {
			let el_stream_image = $("stream-image");
			let to = {
				x: __translate(pos.x, 0, el_stream_image.clientWidth, -32768, 32767),
				y: __translate(pos.y, 0, el_stream_image.clientHeight, -32768, 32767),
			};

			tools.debug("Mouse: moved:", to);
			if (__ws) {
				__ws.send(JSON.stringify({
					event_type: "mouse_move",
					to: to,
				}));
			}
			__sent_pos = pos;
		}
	};

	var __translate = function(x, a, b, c, d) {
		return Math.round((x - a) / (b - a) * (d - c) + c);
	};

	var __streamWheelHandler = function(event) {
		// https://learn.javascript.ru/mousewheel
		if (event.preventDefault) {
			event.preventDefault();
		}

		let delta = {x: 0, y: 0};

		__wheel_delta.y += event.deltaY;
		if (Math.abs(__wheel_delta.y) >= 100) {
			delta.y = __wheel_delta.y / Math.abs(__wheel_delta.y) * (-5);
			__wheel_delta.y = 0;
		}

		if (delta.y) {
			tools.debug("Mouse: scrolled:", delta);
			if (__ws) {
				__ws.send(JSON.stringify({
					event_type: "mouse_wheel",
					delta: delta,
				}));
			}
		}
	};

	__init__();
}
