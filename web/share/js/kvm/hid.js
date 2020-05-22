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


import {tools, $, $$$} from "../tools.js";
import {wm} from "../wm.js";

import {Keyboard} from "./keyboard.js";
import {Mouse} from "./mouse.js";


export function Hid() {
	var self = this;

	/************************************************************************/

	var __keyboard = new Keyboard();
	var __mouse = new Mouse();

	var __init__ = function() {
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
		tools.setOnClick($("hid-reset-button"), __clickResetButton);

		for (let el_shortcut of $$$("[data-shortcut]")) {
			tools.setOnClick(el_shortcut, () => __emitShortcut(el_shortcut.getAttribute("data-shortcut").split(" ")));
		}
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		wm.switchEnabled($("hid-pak-text"), ws);
		wm.switchEnabled($("hid-pak-button"), ws);
		wm.switchEnabled($("hid-reset-button"), ws);
		__keyboard.setSocket(ws);
		__mouse.setSocket(ws);
	};

	self.setState = function(state) {
		__keyboard.setState(state.keyboard);
		__mouse.setState(state.mouse);
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
			let confirm_msg = `
				You're goint to paste ${text.length} characters.<br>
				Are you sure you want to continue?
			`;

			wm.confirm(confirm_msg).then(function(ok) {
				if (ok) {
					wm.switchEnabled($("hid-pak-text"), false);
					wm.switchEnabled($("hid-pak-button"), false);

					tools.debug("HID: paste-as-keys:", text);

					let http = tools.makeRequest("POST", "/api/hid/print?limit=0", function() {
						if (http.readyState === 4) {
							wm.switchEnabled($("hid-pak-text"), true);
							wm.switchEnabled($("hid-pak-button"), true);
							$("hid-pak-text").value = "";
							if (http.status === 413) {
								wm.error("Too many text for paste!");
							} else if (http.status !== 200) {
								wm.error("HID paste error:<br>", http.responseText);
							}
						}
					}, text, "text/plain");
				} else {
					$("hid-pak-text").value = "";
				}
			});
		}
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
