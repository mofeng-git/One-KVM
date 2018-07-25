var keyboard = new function() {
	var __ws = null;

	this.init = function() {
		document.onkeydown = (event) => __keyHandler(event, true);
		document.onkeyup = (event) => __keyHandler(event, false);
	};

	this.setSocket = function(ws) {
		__ws = ws;
		$("hid-keyboard-led").className = (ws ? "led-on" : "led-off");
	};

	var __keyHandler = function(event, state) {
		// https://github.com/wesbos/keycodes/blob/gh-pages/scripts.js
		el_key = $(event.code);
		if (el_key) {
			event.preventDefault();

			tools.debug("Key", (state ? "pressed:" : "released:"), event);

			if (state) {
				el_key.style.boxShadow = "none";
				el_key.style.color = "var(--fg-color-selected)";
				el_key.style.backgroundColor = "var(--bg-color-dark)";
			} else {
				el_key.removeAttribute("style");
			}

			if (__ws) {
				__ws.send(JSON.stringify({
					event_type: "key",
					key: event.code,
					state: state,
				}));
			}
		}
	};
};
