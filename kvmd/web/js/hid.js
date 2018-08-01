var hid = new function() {
	this.init = function() {
		keyboard.init();
		mouse.init();
	};

	this.updateLeds = function() {
		keyboard.updateLeds();
		mouse.updateLeds();
	};

	this.releaseAll = function() {
		keyboard.releaseAll();
	};

	this.emitShortcut = function(...codes) {
		tools.debug("Emitted keys:", codes);
		var delay = 0;
		[[codes, true], [codes.slice().reverse(), false]].forEach(function(op) {
			var [op_codes, state] = op;
			op_codes.forEach(function(code) {
				setTimeout(() => keyboard.fireEvent(code, state), delay);
				delay += 100;
			});
		});
	};

	this.installCapture = function(ws) {
		keyboard.setSocket(ws);
		mouse.setSocket(ws);
	};

	this.clearCapture = function() {
		mouse.setSocket(null);
		keyboard.setSocket(null);
	};
}
