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


function Keypad(keys_parent, key_callback) {
	var self = this;

	/************************************************************************/

	var __merged = {};
	var __keys = {};
	var __modifiers = {};

	var __init__ = function() {
		var code;
		var el_key;

		for (el_key of $$$(keys_parent + " div.key")) {
			code = el_key.getAttribute("data-code");

			tools.setDefault(__keys, code, []);
			__keys[code].push(el_key);

			tools.setDefault(__merged, code, []);
			__merged[code].push(el_key);

			(function(el_key) {
				tools.setOnDown(el_key, () => __clickHandler(el_key, true));
				tools.setOnUp(el_key, () => __clickHandler(el_key, false));
				el_key.onmouseout = function() {
					if (__isPressed(el_key)) {
						__clickHandler(el_key, false);
					}
				};
			})(el_key);
		}

		for (el_key of $$$(keys_parent + " div.modifier")) {
			code = el_key.getAttribute("data-code");

			tools.setDefault(__modifiers, code, []);
			__modifiers[code].push(el_key);

			tools.setDefault(__merged, code, []);
			__merged[code].push(el_key);

			(function(el_key) {
				tools.setOnDown(el_key, () => __toggleModifierHandler(el_key));
			})(el_key);
		}
	};

	/************************************************************************/

	self.releaseAll = function(release_hook=false) {
		for (var dict of [__keys, __modifiers]) {
			for (var code in dict) {
				if (__isActive(dict[code][0])) {
					self.emit(code, false, release_hook);
				}
			}
		}
	};

	self.emit = function(code, state, release_hook=false) {
		if (code in __merged) {
			__commonHandler(__merged[code][0], state, false);
			if (release_hook) {
				for (code in __keys) {
					if (__isActive(__keys[code][0])) {
						self.emit(code, false);
					}
				}
			}
			__unholdModifiers();
		}
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
		for (var code in __modifiers) {
			var el_key = __modifiers[code][0];
			if (__isHolded(el_key)) {
				__deactivate(el_key);
				__process(el_key, false);
			}
		}
	};

	var __isPressed = function(el_key) {
		var is_pressed = false;
		for (el_key of __resolveKeys(el_key)) {
			is_pressed = (is_pressed || el_key.classList.contains("pressed"));
		}
		return is_pressed;
	};

	var __isHolded = function(el_key) {
		var is_holded = false;
		for (el_key of __resolveKeys(el_key)) {
			is_holded = (is_holded || el_key.classList.contains("holded"));
		}
		return is_holded;
	};

	var __isActive = function(el_key) {
		var is_active = false;
		for (el_key of __resolveKeys(el_key)) {
			is_active = (is_active || el_key.classList.contains("pressed") || el_key.classList.contains("holded"));
		}
		return is_active;
	};

	var __activate = function(el_key, cls) {
		for (el_key of __resolveKeys(el_key)) {
			el_key.classList.add(cls);
		}
	};

	var __deactivate = function(el_key) {
		for (el_key of __resolveKeys(el_key)) {
			el_key.classList.remove("pressed");
			el_key.classList.remove("holded");
		}
	};

	var __resolveKeys = function(el_key) {
		var code = el_key.getAttribute("data-code");
		return __merged[code];
	};

	var __process = function(el_key, state) {
		var code = el_key.getAttribute("data-code");
		key_callback(code, state);
	};

	__init__();
}
