var keyboard = new function() {
	var __mac_cmd_hook = ((
		window.navigator.oscpu
		|| window.navigator.platform
		|| window.navigator.appVersion
		|| "Unknown"
	).indexOf("Mac") !== -1);

	var __ws = null;
	var __keys = [];
	var __modifiers = [];

	this.init = function() {
		$("keyboard-window").onkeydown = (event) => __keyboardHandler(event, true);
		$("keyboard-window").onkeyup = (event) => __keyboardHandler(event, false);

		$("stream-window").onkeydown = (event) => __keyboardHandler(event, true);
		$("stream-window").onkeyup = (event) => __keyboardHandler(event, false);

		Array.prototype.forEach.call(document.getElementsByClassName("key"), function(el_key) {
			el_key.onmousedown = () => __clickHandler(el_key, true);
			el_key.onmouseup = () => __clickHandler(el_key, false);
			el_key.onmouseout = function() {
				if (__isPressed(el_key)) {
					__clickHandler(el_key, false);
				}
			};
			__keys.push(el_key);
		});
		Array.prototype.forEach.call(document.getElementsByClassName("modifier"), function(el_key) {
			el_key.onmousedown = () => __toggleModifierHandler(el_key);
			__modifiers.push(el_key);
		});

		if (__mac_cmd_hook) {
			tools.info("Keyboard: enabled Mac-CMD-Hook");
		}
	};

	this.setSocket = function(ws) {
		if (ws !== __ws) {
			keyboard.releaseAll();
			__ws = ws;
		}
		keyboard.updateLeds();
	};

	this.updateLeds = function() {
		var focused = (__ws && (document.activeElement === $("stream-window") || document.activeElement === $("keyboard-window")));
		$("hid-keyboard-led").className = (focused ? "led-on" : "led-off");
	};

	this.releaseAll = function(ws) {
		__keys.concat(__modifiers).forEach(function(el_key) {
			if (__isActive(el_key)) {
				keyboard.fireEvent(el_key.id, false);
			}
		});
	};

	this.fireEvent = function(code, state) {
		$("keyboard-window").dispatchEvent(new KeyboardEvent(
			(state ? "keydown" : "keyup"),
			{code: code},
		));
	};

	var __keyboardHandler = function(event, state) {
		event.preventDefault();
		el_key = $(event.code);
		if (el_key && !event.repeat) {
			__commonHandler(el_key, state, "pressed");
			if (__mac_cmd_hook) {
				// https://bugs.chromium.org/p/chromium/issues/detail?id=28089
				// https://bugzilla.mozilla.org/show_bug.cgi?id=1299553
				if ((event.code === "MetaLeft" || event.code === "MetaRight") && !state) {
					__keys.forEach(function(el_key) {
						if (__isActive(el_key)) {
							// __commonHandler(el_key, false, "pressed");
							keyboard.fireEvent(el_key.id, false);
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
				__sendKey(el_key.id, false);
			}
		});
	};

	var __commonHandler = function(el_key, state, cls) {
		if (state && !__isActive(el_key)) {
			__deactivate(el_key);
			el_key.classList.add(cls);
			__sendKey(el_key.id, true);
		} else {
			__deactivate(el_key);
			__sendKey(el_key.id, false);
		}
	};

	var __isPressed = function(el_key) {
		return el_key.classList.contains("pressed");
	};

	var __isHolded = function(el_key) {
		return el_key.classList.contains("holded");
	};

	var __isActive = function(el_key) {
		return (__isPressed(el_key) || __isHolded(el_key));
	};

	var __deactivate = function(el_key) {
		el_key.classList.remove("pressed");
		el_key.classList.remove("holded");
	};

	var __sendKey = function(code, state) {
		tools.debug("Key", (state ? "pressed:" : "released:"), code);
		if (__ws) {
			__ws.send(JSON.stringify({
				event_type: "key",
				key: code,
				state: state,
			}));
		}
	};
};
