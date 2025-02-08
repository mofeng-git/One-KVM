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


import {tools, $, $$$} from "../tools.js";
import {wm} from "../wm.js";

import {Keyboard} from "./keyboard.js";
import {Mouse} from "./mouse.js";


export function Hid(__getGeometry, __recorder) {
	var self = this;

	/************************************************************************/

	var __state = null;
	var __keyboard = null;
	var __mouse = null;

	var __init__ = function() {
		__keyboard = new Keyboard(__recorder.recordWsEvent);
		__mouse = new Mouse(__getGeometry, __recorder.recordWsEvent);

		let hidden_attr = null;
		let visibility_change_attr = null;

		if (typeof document.hidden !== "undefined") {
			hidden_attr = "hidden";
			visibility_change_attr = "visibilitychange";
		} else if (typeof document.webkitHidden !== "undefined") {
			hidden_attr = "webkitHidden";
			visibility_change_attr = "webkitvisibilitychange";
		} else if (typeof document.mozHidden !== "undefined") {
			hidden_attr = "mozHidden";
			visibility_change_attr = "mozvisibilitychange";
		}

		if (visibility_change_attr) {
			document.addEventListener(
				visibility_change_attr,
				function() {
					if (document[hidden_attr]) {
						__releaseAll();
					}
				},
				false
			);
		}

		window.addEventListener("pagehide", __releaseAll);
		window.addEventListener("blur", __releaseAll);

		tools.el.setOnClick($("hid-connect-switch"), __clickConnectSwitch);
		tools.el.setOnClick($("hid-reset-button"), __clickResetButton);

		for (let el_shortcut of $$$("[data-shortcut]")) {
			tools.el.setOnClick(el_shortcut, function() {
				let ask = false;
				let confirm_id = el_shortcut.getAttribute("data-shortcut-confirm");
				if (confirm_id) {
					ask = $(confirm_id).checked;
				}
				let codes = el_shortcut.getAttribute("data-shortcut").split(" ");
				if (ask) {
					wm.confirm("Do you want to press this hotkey?", codes.join(" + ")).then(function(ok) {
						if (ok) {
							__emitShortcut(codes);
						}
					});
				} else {
					__emitShortcut(codes);
				}
			});
		}

		tools.storage.bindSimpleSwitch($("hid-sysrq-ask-switch"), "hid.sysrq.ask", true);

		tools.el.setOnClick($("hid-jiggler-switch"), __clickJigglerSwitch);
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		if (!ws) {
			self.setState(null);
		}
		__keyboard.setSocket(ws);
		__mouse.setSocket(ws);
	};

	self.setState = function(state) {
		if (state) {
			if (!__state) {
				__state = {"keyboard": {}, "mouse": {}};
			}
			if (state.enabled !== undefined) {
				__state.enabled = state.enabled; // Currently unused, always true
			}
			if (__state.enabled !== undefined) {
				for (let key of ["online", "busy", "connected", "jiggler"]) {
					if (state[key] !== undefined) {
						__state[key] = state[key];
					}
				}
				for (let hid of ["keyboard", "mouse"]) {
					if (state[hid] === undefined) {
						state[hid] = {}; // Add some stubs for processing
					}
					for (let key of ["online", "outputs", (hid === "keyboard" ? "leds" : "absolute")]) {
						__state[hid][key] = state[hid][key];
					}
				}
				if (state.connected !== undefined) {
					tools.feature.setEnabled($("hid-connect"), (__state.connected !== null));
					$("hid-connect-switch").checked = !!__state.connected;
				}
				if (state.jiggler !== undefined) {
					tools.feature.setEnabled($("hid-jiggler"), __state.jiggler.enabled);
					$("hid-jiggler-switch").checked = __state.jiggler.active;
				}
				if (state.keyboard.outputs !== undefined) {
					__updateKeyboardOutputs(__state.keyboard.outputs);
				}
				if (state.mouse.outputs !== undefined) {
					__updateMouseOutputs(__state.mouse.outputs, __state.mouse.absolute); // Follows together
				}
				if (
					state.keyboard.online !== undefined || state.keyboard.leds !== undefined
					|| state.online !== undefined || state.busy !== undefined
				) {
					__keyboard.setState(__state.keyboard.online, __state.keyboard.leds, __state.online, __state.busy);
				}
				if (
					state.mouse.online !== undefined || state.mouse.absolute !== undefined
					|| state.online !== undefined || state.busy !== undefined
				) {
					__mouse.setState(__state.mouse.online, __state.mouse.absolute, __state.online, __state.busy);
				}
				if (state.online !== undefined || state.busy !== undefined) {
					tools.radio.setEnabled("hid-outputs-keyboard-radio", (__state.online && !__state.busy));
					tools.radio.setEnabled("hid-outputs-mouse-radio", (__state.online && !__state.busy));
					tools.el.setEnabled($("hid-connect-switch"), (__state.online && !__state.busy));
				}
			}
		} else {
			__state = null;
			tools.radio.setEnabled("hid-outputs-keyboard-radio", false);
			tools.radio.setEnabled("hid-outputs-mouse-radio", false);
			tools.el.setEnabled($("hid-connect-switch"), false);
			tools.el.setEnabled($("hid-mouse-squash-switch"), false);
			tools.el.setEnabled($("hid-mouse-sens-slider"), false);
		}
		tools.el.setEnabled($("hid-reset-button"), __state);
		tools.el.setEnabled($("hid-jiggler-switch"), __state);
	};

	var __updateKeyboardOutputs = function(outputs) {
		let avail = outputs.available;
		if (avail.length > 0) {
			let el = $("hid-outputs-keyboard-box");
			let avail_json = JSON.stringify(avail);
			if (el.__avail_json !== avail_json) {
				let html = "";
				for (let kv of [
					["USB",  "usb"],
					["PS/2", "ps2"],
					["Off",  "disabled"],
				]) {
					if (avail.includes(kv[1])) {
						html += tools.radio.makeItem("hid-outputs-keyboard-radio", kv[0], kv[1]);
					}
				}
				el.innerHTML = html;
				tools.radio.setOnClick("hid-outputs-keyboard-radio", () => __clickOutputsRadio("keyboard"));
				el.__avail_json = avail_json;
			}
			tools.radio.setValue("hid-outputs-keyboard-radio", outputs.active);
		}
		tools.feature.setEnabled($("hid-outputs-keyboard"), (avail.length > 0));
	};

	var __updateMouseOutputs = function(outputs, absolute) {
		let has_relative = null;
		let has_relative_squash = null;
		let avail = outputs.available;
		if (avail.length > 0) {
			let el = $("hid-outputs-mouse-box");
			let avail_json = JSON.stringify(avail);
			if (el.__avail_json !== avail_json) {
				has_relative = false;
				let html = "";
				for (let kv of [
					["Absolute",  "usb",       false],
					["Abs-Win98", "usb_win98", false],
					["Relative",  "usb_rel",   true],
					["PS/2",      "ps2",       true],
					["Off",       "disabled",  false],
				]) {
					if (avail.includes(kv[1])) {
						html += tools.radio.makeItem("hid-outputs-mouse-radio", kv[0], kv[1]);
						has_relative = (has_relative || kv[2]);
					}
				}
				el.innerHTML = html;
				tools.radio.setOnClick("hid-outputs-mouse-radio", () => __clickOutputsRadio("mouse"));
				el.__avail_json = avail_json;
			}
			tools.radio.setValue("hid-outputs-mouse-radio", outputs.active);
			has_relative_squash = (["usb_rel", "ps2"].includes(outputs.active));
		} else {
			has_relative = !absolute;
			has_relative_squash = has_relative;
		}
		if (has_relative !== null) {
			tools.feature.setEnabled($("hid-mouse-squash"), has_relative);
			tools.feature.setEnabled($("hid-mouse-sens"), has_relative);
		}
		tools.feature.setEnabled($("hid-outputs-mouse"), (avail.length > 0));
		tools.el.setEnabled($("hid-mouse-squash-switch"), has_relative_squash);
		tools.el.setEnabled($("hid-mouse-sens-slider"), has_relative_squash);
	};

	var __releaseAll = function() {
		__keyboard.releaseAll();
		__mouse.releaseAll();
	};

	var __emitShortcut = function(codes) {
		return new Promise(function(resolve) {
			tools.debug("HID: emitting keys:", codes);

			let raw_events = [];
			[[codes, true], [codes.slice().reverse(), false]].forEach(function(op) {
				let [op_codes, state] = op;
				for (let code of op_codes) {
					raw_events.push({"code": code, "state": state});
				}
			});

			let index = 0;
			let iterate = () => setTimeout(function() {
				__keyboard.emit(raw_events[index].code, raw_events[index].state);
				++index;
				if (index < raw_events.length) {
					iterate();
				} else {
					resolve(null);
				}
			}, 100);
			iterate();
		});
	};

	var __clickOutputsRadio = function(hid) {
		let output = tools.radio.getValue(`hid-outputs-${hid}-radio`);
		tools.httpPost("api/hid/set_params", {[`${hid}_output`]: output}, function(http) {
			if (http.status !== 200) {
				wm.error("Can't configure HID", http.responseText);
			}
		});
	};

	var __clickJigglerSwitch = function() {
		let enabled = $("hid-jiggler-switch").checked;
		tools.httpPost("api/hid/set_params", {"jiggler": enabled}, function(http) {
			if (http.status !== 200) {
				wm.error(`Can't ${enabled ? "enabled" : "disable"} mouse jiggler`, http.responseText);
			}
		});
	};

	var __clickConnectSwitch = function() {
		let connected = $("hid-connect-switch").checked;
		tools.httpPost("api/hid/set_connected", {"connected": connected}, function(http) {
			if (http.status !== 200) {
				wm.error(`Can't ${connected ? "connect" : "disconnect"} HID`, http.responseText);
			}
		});
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset HID (keyboard & mouse)?").then(function(ok) {
			if (ok) {
				tools.httpPost("api/hid/reset", null, function(http) {
					if (http.status !== 200) {
						wm.error("HID reset error", http.responseText);
					}
				});
			}
		});
	};

	__init__();
}
