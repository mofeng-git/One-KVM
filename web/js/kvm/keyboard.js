function Keyboard() {
	var self = this;

	/********************************************************************************/

	var __ws = null;

	var __keys = [].slice.call(document.querySelectorAll("div#keyboard-desktop div.keyboard-block div.keyboard-row div.key"));
	var __modifiers = [].slice.call(document.querySelectorAll("div#keyboard-desktop div.keyboard-block div.keyboard-row div.modifier"));

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

		Array.prototype.forEach.call($$("key"), function(el_key) {
			tools.setOnDown(el_key, () => __clickHandler(el_key, true));
			tools.setOnUp(el_key, () => __clickHandler(el_key, false));
			el_key.onmouseout = function() {
				if (__isPressed(el_key)) {
					__clickHandler(el_key, false);
				}
			};
		});

		Array.prototype.forEach.call($$("modifier"), function(el_key) {
			tools.setOnDown(el_key, () => __toggleModifierHandler(el_key));
		});

		if (tools.browser.is_mac) {
			tools.info("Keyboard: enabled Mac-CMD-Hook");
		}
	};

	/********************************************************************************/

	self.setSocket = function(ws) {
		if (ws !== __ws) {
			self.releaseAll();
			__ws = ws;
		}
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
		if (__ws && (document.activeElement === $("stream-window") || document.activeElement === $("keyboard-window"))) {
			$("hid-keyboard-led").className = "led-green";
			$("hid-keyboard-led").title = "Keyboard captured";
		} else {
			$("hid-keyboard-led").className = "led-gray";
			$("hid-keyboard-led").title = "Keyboard free";
		}
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
		Array.prototype.forEach.call(__resolveKeys(el_key), function(el_key) {
			is_pressed = (is_pressed || el_key.classList.contains("pressed"));
		});
		return is_pressed;
	};

	var __isHolded = function(el_key) {
		var is_holded = false;
		Array.prototype.forEach.call(__resolveKeys(el_key), function(el_key) {
			is_holded = (is_holded || el_key.classList.contains("holded"));
		});
		return is_holded;
	};

	var __isActive = function(el_key) {
		var is_active = false;
		Array.prototype.forEach.call(__resolveKeys(el_key), function(el_key) {
			is_active = (is_active || el_key.classList.contains("pressed") || el_key.classList.contains("holded"));
		});
		return is_active;
	};

	var __activate = function(el_key, cls) {
		Array.prototype.forEach.call(__resolveKeys(el_key), function(el_key) {
			el_key.classList.add(cls);
		});
	};

	var __deactivate = function(el_key) {
		Array.prototype.forEach.call(__resolveKeys(el_key), function(el_key) {
			el_key.classList.remove("pressed");
			el_key.classList.remove("holded");
		});
	};

	var __resolveKeys = function(el_key) {
		var code = el_key.getAttribute("data-key");
		return document.querySelectorAll(`[data-key='${code}']`);
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
