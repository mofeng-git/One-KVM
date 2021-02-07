/*****************************************************************************
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
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


import {tools, $, $$$} from "../tools.js";
import {wm} from "../wm.js";

import {Recorder} from "./recorder.js";
import {Keyboard} from "./keyboard.js";
import {Mouse} from "./mouse.js";


export function Hid() {
	var self = this;

	/************************************************************************/

	var __recorder = null;
	var __keyboard = null;
	var __mouse = null;

	var __init__ = function() {
		__recorder = new Recorder();
		__keyboard = new Keyboard(__recorder.recordWsEvent);
		__mouse = new Mouse(__recorder.recordWsEvent);

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

		tools.setOnClick($("hid-pak-button"), __clickPasteAsKeysButton);
		tools.setOnClick($("hid-connect-switch"), __clickConnectSwitch);
		tools.setOnClick($("hid-reset-button"), __clickResetButton);

		for (let el_shortcut of $$$("[data-shortcut]")) {
			tools.setOnClick(el_shortcut, () => __emitShortcut(el_shortcut.getAttribute("data-shortcut").split(" ")));
		}
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		wm.setElementEnabled($("hid-pak-text"), ws);
		wm.setElementEnabled($("hid-pak-button"), ws);
		wm.setElementEnabled($("hid-reset-button"), ws);
		if (!ws) {
			self.setState(null);
		}
		__recorder.setSocket(ws);
		__keyboard.setSocket(ws);
		__mouse.setSocket(ws);
	};

	self.setState = function(state) {
		let has_relative_squash = false;

		if (state && state.online) {
			let keyboard_outputs = state.keyboard.outputs.available;
			let mouse_outputs = state.mouse.outputs.available;
			let has_outputs = (keyboard_outputs.length || mouse_outputs.length);
			let has_relative = false;
			if (has_outputs) {
				if ($("hid-outputs-keyboard").outputs !== keyboard_outputs) {
					let html = "";
					for (let args of [
						["USB", "usb"],
						["PS/2", "ps2"],
						["Off", ""],
					]) {
						if (keyboard_outputs.includes(args[1]) || !args[1]) {
							html += tools.radioMakeItem("hid-outputs-keyboard-radio", args[0], args[1]);
						}
					}
					$("hid-outputs-keyboard").innerHTML = html;
					$("hid-outputs-keyboard").outputs = keyboard_outputs;
					tools.radioSetOnClick("hid-outputs-keyboard-radio", () => __clickOutputsRadio("keyboard"));
				}
				if ($("hid-outputs-mouse").outputs !== mouse_outputs) {
					let html = "";
					for (let args of [
						["USB", "usb", false],
						["USB Relative", "usb_rel", true],
						["PS/2", "ps2", true],
						["Off", ""],
					]) {
						if (mouse_outputs.includes(args[1]) || !args[1]) {
							html += tools.radioMakeItem("hid-outputs-mouse-radio", args[0], args[1]);
							has_relative = (has_relative || args[2]);
						}
					}
					$("hid-outputs-mouse").innerHTML = html;
					$("hid-outputs-mouse").outputs = mouse_outputs;
					tools.radioSetOnClick("hid-outputs-mouse-radio", () => __clickOutputsRadio("mouse"));
				}
				tools.radioSetValue("hid-outputs-keyboard-radio", state.keyboard.outputs.active);
				tools.radioSetValue("hid-outputs-mouse-radio", state.mouse.outputs.active);
				has_relative_squash = ["usb_rel", "ps2"].includes(state.mouse.outputs.active);
			} else {
				has_relative = !state.mouse.absolute;
				has_relative_squash = has_relative;
			}
			tools.featureSetEnabled($("hid-outputs"), has_outputs);
			tools.featureSetEnabled($("hid-mouse-squash"), has_relative);
			tools.featureSetEnabled($("hid-connect"), (state.connected !== null));
			$("hid-connect-switch").checked = !!state.connected;
		}

		wm.setRadioEnabled("hid-outputs-keyboard-radio", (state && state.online && !state.busy));
		wm.setRadioEnabled("hid-outputs-mouse-radio", (state && state.online && !state.busy));
		wm.setElementEnabled($("hid-mouse-squash-switch"), (has_relative_squash && !state.busy));
		wm.setElementEnabled($("hid-connect-switch"), (state && state.online && !state.busy));

		if (state) {
			__keyboard.setState(state.keyboard, state.online, state.busy);
			__mouse.setState(state.mouse, state.online, state.busy);
		}
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
					raw_events.push({code: code, state: state});
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
			}, 50);
			iterate();
		});
	};

	var __clickPasteAsKeysButton = function() {
		let text = $("hid-pak-text").value.replace(/[^\x00-\x7F]/g, "");  // eslint-disable-line no-control-regex
		if (text) {
			let confirm_msg = `You're going to paste ${text.length} character${text.length ? "s" : ""}.<br>`;
			confirm_msg += "Are you sure you want to continue?";

			wm.confirm(confirm_msg).then(function(ok) {
				if (ok) {
					wm.setElementEnabled($("hid-pak-text"), false);
					wm.setElementEnabled($("hid-pak-button"), false);

					tools.debug("HID: paste-as-keys:", text);

					let http = tools.makeRequest("POST", "/api/hid/print?limit=0", function() {
						if (http.readyState === 4) {
							wm.setElementEnabled($("hid-pak-text"), true);
							wm.setElementEnabled($("hid-pak-button"), true);
							$("hid-pak-text").value = "";
							if (http.status === 413) {
								wm.error("Too many text for paste!");
							} else if (http.status !== 200) {
								wm.error("HID paste error:<br>", http.responseText);
							} else if (http.status === 200) {
								__recorder.recordPrintEvent(text);
							}
						}
					}, text, "text/plain");
				} else {
					$("hid-pak-text").value = "";
				}
			});
		}
	};

	var __clickOutputsRadio = function(hid) {
		let output = tools.radioGetValue(`hid-outputs-${hid}-radio`);
		let http = tools.makeRequest("POST", `/api/hid/set_params?${hid}_output=${output}`, function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					wm.error("Can't configure HID:<br>", http.responseText);
				}
			}
		});
	};

	var __clickConnectSwitch = function() {
		let connected = $("hid-connect-switch").checked;
		let http = tools.makeRequest("POST", `/api/hid/set_connected?connected=${connected}`, function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					wm.error(`Can't ${connected ? "connect" : "disconnect"} HID:<br>`, http.responseText);
				}
			}
		});
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset HID (keyboard & mouse)?").then(function(ok) {
			if (ok) {
				let http = tools.makeRequest("POST", "/api/hid/reset", function() {
					if (http.readyState === 4) {
						if (http.status !== 200) {
							wm.error("HID reset error:<br>", http.responseText);
						}
					}
				});
			}
		});
	};

	__init__();
}
