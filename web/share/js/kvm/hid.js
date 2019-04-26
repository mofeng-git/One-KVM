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


function Hid() {
	var self = this;

	/************************************************************************/

	var __ws = null;

	var __chars_to_codes = {};
	var __codes_delay = 50;

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

		__chars_to_codes = __buildCharsToCodes();

		tools.setOnClick($("hid-pak-button"), __clickPasteAsKeysButton);
		tools.setOnClick($("hid-reset-button"), __clickResetButton);

		for (let el_shortcut of $$$("[data-shortcut]")) {
			tools.setOnClick(el_shortcut, () => __emitShortcut(el_shortcut.getAttribute("data-shortcut").split(" ")));
		}
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		wm.switchDisabled($("hid-pak-text"), !ws);
		wm.switchDisabled($("hid-pak-button"), !ws);
		wm.switchDisabled($("hid-reset-button"), !ws);
		__ws = ws;
		__keyboard.setSocket(ws);
		__mouse.setSocket(ws);
	};

	self.setState = function(state) {
		__keyboard.setState(state);
		__mouse.setState(state);
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
			}, __codes_delay);
			iterate();
		});
	};

	var __buildCharsToCodes = function() {
		let chars_to_codes = {
			"\n": ["Enter"],
			"\t": ["Tab"],
			" ": ["Space"],
			"`": ["Backquote"],   "~": ["ShiftLeft", "Backquote"],
			"\\": ["Backslash"],  "|": ["ShiftLeft", "Backslash"],
			"[": ["BracketLeft"], "{": ["ShiftLeft", "BracketLeft"],
			"]": ["BracketLeft"], "}": ["ShiftLeft", "BracketRight"],
			",": ["Comma"],       "<": ["ShiftLeft", "Comma"],
			".": ["Period"],      ">": ["ShiftLeft", "Period"],
			"1": ["Digit1"],      "!": ["ShiftLeft", "Digit1"],
			"2": ["Digit2"],      "@": ["ShiftLeft", "Digit2"],
			"3": ["Digit3"],      "#": ["ShiftLeft", "Digit3"],
			"4": ["Digit4"],      "$": ["ShiftLeft", "Digit4"],
			"5": ["Digit5"],      "%": ["ShiftLeft", "Digit5"],
			"6": ["Digit6"],      "^": ["ShiftLeft", "Digit6"],
			"7": ["Digit7"],      "&": ["ShiftLeft", "Digit7"],
			"8": ["Digit8"],      "*": ["ShiftLeft", "Digit8"],
			"9": ["Digit9"],      "(": ["ShiftLeft", "Digit9"],
			"0": ["Digit0"],      ")": ["ShiftLeft", "Digit0"],
			"-": ["Minus"],       "_": ["ShiftLeft", "Minus"],
			"'": ["Quote"],       "\"": ["ShiftLeft", "Quote"],
			";": ["Semicolon"],   ":": ["ShiftLeft", "Semicolon"],
			"/": ["Slash"],       "?": ["ShiftLeft", "Slash"],
			"=": ["Equal"],       "+": ["ShiftLeft", "Equal"],
		};

		for (let ch = "a".charCodeAt(0); ch <= "z".charCodeAt(0); ++ch) {
			let low = String.fromCharCode(ch);
			let up = low.toUpperCase();
			let code = "Key" + up;
			chars_to_codes[low] = [code];
			chars_to_codes[up] = ["ShiftLeft", code];
		}

		return chars_to_codes;
	};

	var __clickPasteAsKeysButton = function() {
		let text = $("hid-pak-text").value.replace(/[^\x00-\x7F]/g, "");  // eslint-disable-line no-control-regex
		if (text) {
			let clipboard_codes = [];
			let codes_count = 0;
			for (let ch of text) {
				let codes = __chars_to_codes[ch];
				if (codes) {
					codes_count += codes.length;
					clipboard_codes.push(codes);
				}
			}
			let time = __codes_delay * codes_count * 2 / 1000;

			let confirm_msg = `
				You are going to automatically type ${codes_count} characters from the system clipboard.
				It will take ${time} seconds.<br>
				<br>
				Are you sure you want to continue?
			`;

			wm.confirm(confirm_msg).then(function(ok) {
				if (ok) {
					wm.switchDisabled($("hid-pak-text"), true);
					wm.switchDisabled($("hid-pak-button"), true);
					$("hid-pak-led").className = "led-yellow-rotating-fast";
					$("hid-pak-led").title = "Autotyping...";

					tools.debug("HID: paste-as-keys:", text);

					let index = 0;
					let iterate = function() {
						__emitShortcut(clipboard_codes[index]).then(function() {
							++index;
							if (index < clipboard_codes.length && __ws) {
								iterate();
							} else {
								$("hid-pak-text").value = "";
								wm.switchDisabled($("hid-pak-text"), false);
								wm.switchDisabled($("hid-pak-button"), false);
								$("hid-pak-led").className = "led-gray";
								$("hid-pak-led").title = "";
							}
						});
					};
					iterate();
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
