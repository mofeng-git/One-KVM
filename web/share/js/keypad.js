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


import {tools, $$$} from "./tools.js";


export function Keypad(__keys_parent, __sendKey, __apply_fixes) {
	var self = this;

	/************************************************************************/

	var __merged = {};
	var __keys = {};
	var __modifiers = {};

	var __fix_mac_cmd = false;
	var __fix_win_altgr = false;
	var __altgr_ctrl_timer = null;

	var __init__ = function() {
		if (__apply_fixes) {
			__fix_mac_cmd = tools.browser.is_mac;
			if (__fix_mac_cmd) {
				tools.info(`Keymap at ${__keys_parent}: enabled Fix-Mac-CMD`);
			}
			__fix_win_altgr = tools.browser.is_win;
			if (__fix_win_altgr) {
				tools.info(`Keymap at ${__keys_parent}: enabled Fix-Win-AltGr`);
			}
		}

		for (let el_key of $$$(`${__keys_parent} div.key`)) {
			let code = el_key.getAttribute("data-code");

			tools.setDefault(__keys, code, []);
			__keys[code].push(el_key);

			tools.setDefault(__merged, code, []);
			__merged[code].push(el_key);

			tools.el.setOnDown(el_key, () => __clickHandler(el_key, true));
			tools.el.setOnUp(el_key, () => __clickHandler(el_key, false));
			el_key.onmouseout = function() {
				if (__isPressed(el_key)) {
					__clickHandler(el_key, false);
				}
			};
		}

		for (let el_key of $$$(`${__keys_parent} div.modifier`)) {
			let code = el_key.getAttribute("data-code");

			tools.setDefault(__modifiers, code, []);
			__modifiers[code].push(el_key);

			tools.setDefault(__merged, code, []);
			__merged[code].push(el_key);

			tools.el.setOnDown(el_key, () => __toggleModifierHandler(el_key));
		}
	};

	/************************************************************************/

	self.releaseAll = function() {
		for (let dict of [__keys, __modifiers]) {
			for (let code in dict) {
				if (__isActive(dict[code][0])) {
					self.emitByCode(code, false);
				}
			}
		}
	};

	self.emitByKeyEvent = function(event, state) {
		if (event.repeat) {
			return;
		}

		let code = event.code;
		if (__apply_fixes) {
			// https://github.com/pikvm/pikvm/issues/819
			if (code == "IntlBackslash" && ["`", "~"].includes(event.key)) {
				code = "Backquote";
			} else if (code == "Backquote" && ["§", "±"].includes(event.key)) {
				code = "IntlBackslash";
			}
		}

		self.emitByCode(code, state);
	};

	self.emitByCode = function(code, state, apply_fixes=true) {
		if (code in __merged) {
			if (__fix_win_altgr && apply_fixes) {
				if (!__fixWinAltgr(code, state)) {
					return;
				}
			}
			if (__fix_mac_cmd && apply_fixes) {
				__fixMacCmd(code, state);
			}
			__commonHandler(__merged[code][0], state, false);
			__unholdModifiers();
		}
	};

	var __fixMacCmd = function(code, state) {
		if ((code == "MetaLeft" || code == "MetaRight") && !state) {
			for (code in __keys) {
				if (__isActive(__keys[code][0])) {
					self.emitByCode(code, false, false);
				}
			}
		}
	};

	var __fixWinAltgr = function(code, state) {
		// https://github.com/pikvm/pikvm/issues/375
		// https://github.com/novnc/noVNC/blob/84f102d6/core/input/keyboard.js
		if (state) {
			if (__altgr_ctrl_timer) {
				clearTimeout(__altgr_ctrl_timer);
				__altgr_ctrl_timer = null;
				if (code !== "AltRight") {
					self.emitByCode("ControlLeft", true, false);
				}
			}
			if (code === "ControlLeft" && !__isActive(__modifiers["ControlLeft"][0])) {
				__altgr_ctrl_timer = setTimeout(function() {
					__altgr_ctrl_timer = null;
					self.emitByCode("ControlLeft", true, false);
				}, 50);
				return false; // Stop handling
			}
		} else {
			if (__altgr_ctrl_timer) {
				clearTimeout(__altgr_ctrl_timer);
				__altgr_ctrl_timer = null;
				self.emitByCode("ControlLeft", true, false);
			}
		}
		return true; // Continue handling
	};

	var __clickHandler = function(el_key, state) {
		__commonHandler(el_key, state, false);
		__unholdModifiers();
	};

	var __toggleModifierHandler = function(el_key) {
		__commonHandler(el_key, !__isActive(el_key), true);
	};

	var __commonHandler = function(el_key, state, hold) {
		if (state && !__isActive(el_key)) {
			__deactivate(el_key);
			__activate(el_key, (hold ? "holded" : "pressed"));
			__process(el_key, true);
		} else {
			__deactivate(el_key);
			__process(el_key, false);
		}
	};

	var __unholdModifiers = function() {
		for (let code in __modifiers) {
			let el_key = __modifiers[code][0];
			if (__isHolded(el_key)) {
				__deactivate(el_key);
				__process(el_key, false);
			}
		}
	};

	var __isPressed = function(el_key) {
		let is_pressed = false;
		let el_keys = __resolveKeys(el_key);
		for (el_key of el_keys) {
			is_pressed = (is_pressed || el_key.classList.contains("pressed"));
		}
		return is_pressed;
	};

	var __isHolded = function(el_key) {
		let is_holded = false;
		let el_keys = __resolveKeys(el_key);
		for (el_key of el_keys) {
			is_holded = (is_holded || el_key.classList.contains("holded"));
		}
		return is_holded;
	};

	var __isActive = function(el_key) {
		let is_active = false;
		let el_keys = __resolveKeys(el_key);
		for (el_key of el_keys) {
			is_active = (is_active || el_key.classList.contains("pressed") || el_key.classList.contains("holded"));
		}
		return is_active;
	};

	var __activate = function(el_key, cls) {
		let el_keys = __resolveKeys(el_key);
		for (el_key of el_keys) {
			el_key.classList.add(cls);
		}
	};

	var __deactivate = function(el_key) {
		let el_keys = __resolveKeys(el_key);
		for (el_key of el_keys) {
			el_key.classList.remove("pressed");
			el_key.classList.remove("holded");
		}
	};

	var __resolveKeys = function(el_key) {
		let code = el_key.getAttribute("data-code");
		return __merged[code];
	};

	var __process = function(el_key, state) {
		let code = el_key.getAttribute("data-code");
		__sendKey(code, state);
	};

	__init__();
}
