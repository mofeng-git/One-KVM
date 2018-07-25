var keyboard = new function() {
	this.installCapture = function(ws) {
		// https://www.codeday.top/2017/05/03/24906.html
		tools.info("Installing keyboard capture ...")
		document.onkeydown = (event) => __keyHandler(ws, event, true);
		document.onkeyup = (event) => __keyHandler(ws, event, false);
		$("hid-keyboard-led").className = "led-on";
	};

	this.clearCapture = function() {
		tools.info("Removing keyboard capture ...")
		document.onkeydown = null;
		document.onkeyup = null;
		$("hid-keyboard-led").className = "led-off";
	};

	var __keyHandler = function(ws, event, state) {
		// https://github.com/wesbos/keycodes/blob/gh-pages/scripts.js
		el_key = $(event.code);
		if (el_key) {
			tools.debug("Key", (state ? "pressed:" : "released:"), event);

			if (state) {
				el_key.style.boxShadow = "none";
				el_key.style.color = "var(--fg-color-selected)";
				el_key.style.backgroundColor = "var(--bg-color-dark)";
			} else {
				el_key.removeAttribute("style");
			}

			event.preventDefault();
			ws.send(JSON.stringify({
				event_type: "key",
				key: event.code,
				state: state,
			}));
		}
	};
};
