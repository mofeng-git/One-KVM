var keyboard = new function() {
	var __ws = null;
	var __keys = [];
	var __modifiers = [];

	this.init = function() {
		document.onkeydown = (event) => __keyboardHandler(event, true);
		document.onkeyup = (event) => __keyboardHandler(event, false);

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
	};

	this.setSocket = function(ws) {
		keyboard.releaseAll();
		__ws = ws;
		$("hid-keyboard-led").className = (ws ? "led-on" : "led-off");
	};

	this.releaseAll = function(ws) {
		__keys.concat(__modifiers).forEach(function(el_key) {
			if (__isActive(el_key)) {
				keyboard.fireEvent(el_key.id, false);
			}
		});
	};

	this.fireEvent = function(code, state) {
		document.dispatchEvent(new KeyboardEvent(
			(state ? "keydown" : "keyup"),
			{code: code},
		));
	};

	var __keyboardHandler = function(event, state) {
		event.preventDefault();
		el_key = $(event.code);
		if (el_key && !event.repeat) {
			__commonHandler(el_key, state, "pressed");
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
