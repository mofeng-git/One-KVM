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

		tools.storage.bindSimpleSwitch($("hid-pak-ask-switch"), "hid.pak.ask", true);
		tools.storage.bindSimpleSwitch($("hid-pak-secure-switch"), "hid.pak.secure", false, function(value) {
			$("hid-pak-text").style.setProperty("-webkit-text-security", (value ? "disc" : "none"));
		});
		tools.feature.setEnabled($("hid-pak-secure"), (
			tools.browser.is_chrome
			|| tools.browser.is_safari
			|| tools.browser.is_opera
		));

		$("hid-pak-keymap-selector").addEventListener("change", function() {
			tools.storage.set("hid.pak.keymap", $("hid-pak-keymap-selector").value);
		});

		tools.el.setOnClick($("hid-pak-button"), __clickPasteAsKeysButton);
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
					let confirm_msg = `Do you want to press <b>${codes.join(" + ")}</b>?`;
					wm.confirm(confirm_msg).then(function(ok) {
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
		tools.el.setEnabled($("hid-pak-text"), ws);
		tools.el.setEnabled($("hid-pak-button"), ws);
		tools.el.setEnabled($("hid-reset-button"), ws);
		tools.el.setEnabled($("hid-jiggler-switch"), ws);
		if (!ws) {
			self.setState(null);
		}
		__keyboard.setSocket(ws);
		__mouse.setSocket(ws);
	};

	self.setState = function(state) {
		let has_relative_squash = false;

		if (state) {
			tools.feature.setEnabled($("hid-jiggler"), state.jiggler.enabled);
			$("hid-jiggler-switch").checked = state.jiggler.active;
		}
		if (state && state.online) {
			let keyboard_outputs = state.keyboard.outputs.available;
			let mouse_outputs = state.mouse.outputs.available;
			if (keyboard_outputs.length) {
				if ($("hid-outputs-keyboard-box").outputs !== keyboard_outputs) {
					let html = "";
					for (let args of [
						["USB", "usb"],
						["PS/2", "ps2"],
						["Off", "disabled"],
					]) {
						if (keyboard_outputs.includes(args[1])) {
							html += tools.radio.makeItem("hid-outputs-keyboard-radio", args[0], args[1]);
						}
					}
					$("hid-outputs-keyboard-box").innerHTML = html;
					$("hid-outputs-keyboard-box").outputs = keyboard_outputs;
					tools.radio.setOnClick("hid-outputs-keyboard-radio", () => __clickOutputsRadio("keyboard"));
				}
				tools.radio.setValue("hid-outputs-keyboard-radio", state.keyboard.outputs.active);
			}
			let has_relative = false;
			if (mouse_outputs.length) {
				if ($("hid-outputs-mouse-box").outputs !== mouse_outputs) {
					let html = "";
					for (let args of [
						["Absolute", "usb", false],
						["Abs-Win98", "usb_win98", false],
						["Relative", "usb_rel", true],
						["PS/2", "ps2", true],
						["Off", "disabled"],
					]) {
						if (mouse_outputs.includes(args[1])) {
							html += tools.radio.makeItem("hid-outputs-mouse-radio", args[0], args[1]);
							has_relative = (has_relative || args[2]);
						}
					}
					$("hid-outputs-mouse-box").innerHTML = html;
					$("hid-outputs-mouse-box").outputs = mouse_outputs;
					tools.radio.setOnClick("hid-outputs-mouse-radio", () => __clickOutputsRadio("mouse"));
				}
				tools.radio.setValue("hid-outputs-mouse-radio", state.mouse.outputs.active);
				has_relative_squash = ["usb_rel", "ps2"].includes(state.mouse.outputs.active);
			} else {
				has_relative = !state.mouse.absolute;
				has_relative_squash = has_relative;
			}
			tools.feature.setEnabled($("hid-outputs"), (keyboard_outputs.length || mouse_outputs.length));
			tools.feature.setEnabled($("hid-outputs-keyboard"), keyboard_outputs.length);
			tools.feature.setEnabled($("hid-outputs-mouse"), mouse_outputs.length);
			tools.feature.setEnabled($("hid-mouse-squash"), has_relative);
			tools.feature.setEnabled($("hid-mouse-sens"), has_relative);
			tools.feature.setEnabled($("hid-connect"), (state.connected !== null));
			$("hid-connect-switch").checked = !!state.connected;
		}

		tools.radio.setEnabled("hid-outputs-keyboard-radio", (state && state.online && !state.busy));
		tools.radio.setEnabled("hid-outputs-mouse-radio", (state && state.online && !state.busy));
		tools.el.setEnabled($("hid-mouse-squash-switch"), (has_relative_squash && !state.busy));
		tools.el.setEnabled($("hid-mouse-sens-slider"), (has_relative_squash && !state.busy));
		tools.el.setEnabled($("hid-connect-switch"), (state && state.online && !state.busy));

		if (state) {
			__keyboard.setState(state.keyboard, state.online, state.busy);
			__mouse.setState(state.mouse, state.online, state.busy);
		}
	};

	self.setKeymaps = function(state) {
		let el = $("hid-pak-keymap-selector");
		tools.selector.setValues(el, state.keymaps.available);
		tools.selector.setSelectedValue(el, tools.storage.get("hid.pak.keymap", state.keymaps["default"]));
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

	var __clickPasteAsKeysButton = function() {
		let text = $("hid-pak-text").value;
		if (text) {
			let paste_as_keys = function() {
				tools.el.setEnabled($("hid-pak-text"), false);
				tools.el.setEnabled($("hid-pak-button"), false);
				tools.el.setEnabled($("hid-pak-keymap-selector"), false);

				let keymap = $("hid-pak-keymap-selector").value;

				tools.debug(`HID: paste-as-keys ${keymap}: ${text}`);

				tools.httpPost(`/api/hid/print?limit=0&keymap=${keymap}`, function(http) {
					tools.el.setEnabled($("hid-pak-text"), true);
					tools.el.setEnabled($("hid-pak-button"), true);
					tools.el.setEnabled($("hid-pak-keymap-selector"), true);
					$("hid-pak-text").value = "";
					if (http.status === 413) {
						wm.error("Too many text for paste!");
					} else if (http.status !== 200) {
						wm.error("HID paste error:<br>", http.responseText);
					} else if (http.status === 200) {
						__recorder.recordPrintEvent(text);
					}
				}, text, "text/plain");
			};

			if ($("hid-pak-ask-switch").checked) {
				let confirm_msg = `You're going to paste ${text.length} character${text.length ? "s" : ""}.<br>`;
				confirm_msg += "Are you sure you want to continue?";
				wm.confirm(confirm_msg).then(function(ok) {
					if (ok) {
						paste_as_keys();
					} else {
						$("hid-pak-text").value = "";
					}
				});
			} else {
				paste_as_keys();
			}
		}
	};

	var __clickOutputsRadio = function(hid) {
		let output = tools.radio.getValue(`hid-outputs-${hid}-radio`);
		tools.httpPost(`/api/hid/set_params?${hid}_output=${output}`, function(http) {
			if (http.status !== 200) {
				wm.error("Can't configure HID:<br>", http.responseText);
			}
		});
	};

	var __clickJigglerSwitch = function() {
		let enabled = $("hid-jiggler-switch").checked;
		tools.httpPost(`/api/hid/set_params?jiggler=${enabled}`, function(http) {
			if (http.status !== 200) {
				wm.error(`Can't ${enabled ? "enabled" : "disable"} mouse juggler:<br>`, http.responseText);
			}
		});
	};

	var __clickConnectSwitch = function() {
		let connected = $("hid-connect-switch").checked;
		tools.httpPost(`/api/hid/set_connected?connected=${connected}`, function(http) {
			if (http.status !== 200) {
				wm.error(`Can't ${connected ? "connect" : "disconnect"} HID:<br>`, http.responseText);
			}
		});
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset HID (keyboard & mouse)?").then(function(ok) {
			if (ok) {
				tools.httpPost("/api/hid/reset", function(http) {
					if (http.status !== 200) {
						wm.error("HID reset error:<br>", http.responseText);
					}
				});
			}
		});
	};

	__init__();
}
