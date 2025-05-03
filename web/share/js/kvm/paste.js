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
import {wm} from "../wm.js";


export function Paste(__recorder) {
	var self = this;

	/************************************************************************/

	var __init__ = function() {
		$("hid-pak-text").addEventListener("keyup", function(event) {
			if (event.ctrlKey && event.code == "Enter") {
				$("hid-pak-button").click();
			}
		});

		tools.storage.bindSimpleSwitch($("hid-pak-ask-switch"), "hid.pak.ask", true);
		tools.storage.bindSimpleSwitch($("hid-pak-slow-switch"), "hid.pak.slow", false);

		tools.storage.bindSimpleSwitch($("hid-pak-secure-switch"), "hid.pak.secure", false, function(value) {
			$("hid-pak-text").style.setProperty("-webkit-text-security", (value ? "disc" : "none"));
		});

		$("hid-pak-keymap-selector").addEventListener("change", function() {
			tools.storage.set("hid.pak.keymap", $("hid-pak-keymap-selector").value);
		});

		tools.el.setOnClick($("hid-pak-button"), __clickPasteAsKeysButton);
	};

	/************************************************************************/

	self.setState = function(state) {
		tools.el.setEnabled($("hid-pak-text"), state);
		tools.el.setEnabled($("hid-pak-button"), state);
		if (state) {
			let el = $("hid-pak-keymap-selector");
			let sel = tools.storage.get("hid.pak.keymap", state.keymaps["default"]);
			el.options.length = 0;
			for (let keymap of state.keymaps.available) {
				tools.selector.addOption(el, keymap, keymap, (keymap === sel));
			}
		}
	};

	var __clickPasteAsKeysButton = function() {
		let text = $("hid-pak-text").value;
		if (text) {
			let paste_as_keys = function() {
				tools.el.setEnabled($("hid-pak-text"), false);
				tools.el.setEnabled($("hid-pak-button"), false);
				tools.el.setEnabled($("hid-pak-keymap-selector"), false);

				let keymap = $("hid-pak-keymap-selector").value;
				let slow = $("hid-pak-slow-switch").checked;

				tools.debug(`HID: paste-as-keys ${keymap}: ${text}`);

				tools.httpPost("api/hid/print", {"limit": 0, "keymap": keymap, "slow": slow}, function(http) {
					tools.el.setEnabled($("hid-pak-text"), true);
					tools.el.setEnabled($("hid-pak-button"), true);
					tools.el.setEnabled($("hid-pak-keymap-selector"), true);
					$("hid-pak-text").value = "";
					if (http.status === 413) {
						wm.error("Too many text for paste!");
					} else if (http.status !== 200) {
						wm.error("HID paste error", http.responseText);
					} else if (http.status === 200) {
						__recorder.recordPrintEvent(text, keymap, slow);
					}
				}, text, "text/plain", 7 * 24 * 3600);
			};

			if ($("hid-pak-ask-switch").checked) {
				wm.confirm(`
					You're going to paste ${text.length} character${text.length ? "s" : ""}.<br>
					Are you sure you want to continue?
				`).then(function(ok) {
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

	__init__();
}
