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


function Keyboard() {
	var self = this;

	/************************************************************************/

	var __ws = null;
	var __ok = true;

	var __keys = [].slice.call($$$("div#keyboard-desktop div.keyboard-block div.keyboard-row div.key"));
	var __modifiers = [].slice.call($$$("div#keyboard-desktop div.keyboard-block div.keyboard-row div.modifier"));

	var __init__ = function() {
		$("hid-keyboard-led").title = "Keyboard free";

		$("keyboard-window").onkeydown = (event) => __keyboardHandler(event, true);
		$("keyboard-window").onkeyup = (event) => __keyboardHandler(event, false);
		$("keyboard-window").onfocus = __updateLeds;
		$("keyboard-window").onblur = __updateLeds;

		$("stream-window").onkeydown = (event) => __keyboardHandler(event, true);
		$("stream-window").onkeyup = (event) => __keyboardHandler(event, false);
		$("stream-window").onfocus = __updateLeds;
		$("stream-window").onblur = __updateLeds;

		window.addEventListener("focusin", __updateLeds);
		window.addEventListener("focusout", __updateLeds);

		tools.forEach($$("key"), function(el_key) {
			tools.setOnDown(el_key, () => __clickHandler(el_key, true));
			tools.setOnUp(el_key, () => __clickHandler(el_key, false));
			el_key.onmouseout = function() {
				if (__isPressed(el_key)) {
					__clickHandler(el_key, false);
				}
			};
		});

		tools.forEach($$("modifier"), function(el_key) {
			tools.setOnDown(el_key, () => __toggleModifierHandler(el_key));
		});

		if (tools.browser.is_mac) {
			tools.info("Keyboard: enabled Mac-CMD-Hook");
		}
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		if (ws !== __ws) {
			self.releaseAll();
			__ws = ws;
		}
		__updateLeds();
	};

	self.setState = function(state) {
		__ok = state.ok;
		__updateLeds();
	};

	self.releaseAll = function() {
		__keys.concat(__modifiers).forEach(function(el_key) {
			if (__isActive(el_key)) {
				self.fireEvent(el_key.getAttribute("data-key"), false);
			}
		});
	};

	self.fireEvent = function(code, state) {
		__keyboardHandler({code: code}, state);
	};

	var __updateLeds = function() {
		var is_captured = (
			$("stream-window").classList.contains("window-active")
			|| $("keyboard-window").classList.contains("window-active")
		);
		var led = "led-gray";
		var title = "Keyboard free";

		if (__ws) {
			if (__ok) {
				if (is_captured) {
					led = "led-green";
					title = "Keyboard captured";
				}
			} else {
				led = "led-yellow";
				title = (is_captured ? "Keyboard captured, HID offline" : "Keyboard free, HID offline");
			}
		} else {
			if (is_captured) {
				title = "Keyboard captured, Pi-KVM offline";
			}
		}
		$("hid-keyboard-led").className = led;
		$("hid-keyboard-led").title = title;
	};

	var __keyboardHandler = function(event, state) {
		if (event.preventDefault) {
			event.preventDefault();
		}
		var el_key = document.querySelector(`[data-key='${event.code}']`);
		if (el_key && !event.repeat) {
			__commonHandler(el_key, state, "pressed");
			if (tools.browser.is_mac) {
				// https://bugs.chromium.org/p/chromium/issues/detail?id=28089
				// https://bugzilla.mozilla.org/show_bug.cgi?id=1299553
				if ((event.code === "MetaLeft" || event.code === "MetaRight") && !state) {
					__keys.forEach(function(el_key) {
						if (__isActive(el_key)) {
							self.fireEvent(el_key.getAttribute("data-key"), false);
						}
					});
				}
			}
			__unholdModifiers();
		}
	};

	var __clickHandler = function(el_key, state) {
		__commonHandler(el_key, state, "pressed");
		__unholdModifiers();
	};

	var __toggleModifierHandler = function(el_key) {
		__commonHandler(el_key, !__isActive(el_key), "holded");
	};

	var __unholdModifiers = function() {
		__modifiers.forEach(function(el_key) {
			if (__isHolded(el_key)) {
				__deactivate(el_key);
				__sendKey(el_key, false);
			}
		});
	};

	var __commonHandler = function(el_key, state, cls) {
		if (state && !__isActive(el_key)) {
			__deactivate(el_key);
			__activate(el_key, cls);
			__sendKey(el_key, true);
		} else {
			__deactivate(el_key);
			__sendKey(el_key, false);
		}
	};

	var __isPressed = function(el_key) {
		var is_pressed = false;
		tools.forEach(__resolveKeys(el_key), function(el_key) {
			is_pressed = (is_pressed || el_key.classList.contains("pressed"));
		});
		return is_pressed;
	};

	var __isHolded = function(el_key) {
		var is_holded = false;
		tools.forEach(__resolveKeys(el_key), function(el_key) {
			is_holded = (is_holded || el_key.classList.contains("holded"));
		});
		return is_holded;
	};

	var __isActive = function(el_key) {
		var is_active = false;
		tools.forEach(__resolveKeys(el_key), function(el_key) {
			is_active = (is_active || el_key.classList.contains("pressed") || el_key.classList.contains("holded"));
		});
		return is_active;
	};

	var __activate = function(el_key, cls) {
		tools.forEach(__resolveKeys(el_key), function(el_key) {
			el_key.classList.add(cls);
		});
	};

	var __deactivate = function(el_key) {
		tools.forEach(__resolveKeys(el_key), function(el_key) {
			el_key.classList.remove("pressed");
			el_key.classList.remove("holded");
		});
	};

	var __resolveKeys = function(el_key) {
		var code = el_key.getAttribute("data-key");
		return $$$(`[data-key='${code}']`);
	};

	var __sendKey = function(el_key, state) {
		var code = el_key.getAttribute("data-key");
		tools.debug("Keyboard: key", (state ? "pressed:" : "released:"), code);
		if (__ws) {
			__ws.send(JSON.stringify({
				event_type: "key",
				key: code,
				state: state,
			}));
		}
	};

	__init__();
}
