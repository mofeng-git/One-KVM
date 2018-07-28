var hid = new function() {
	var __install_timer = null;
	var __installed = false;

	var __hidden_attr = null;
	var __visibility_change_attr = null;

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
		var http = tools.makeRequest("GET", "/kvmd/hid", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					features = JSON.parse(http.responseText).result.features;
					if (features.mouse) {
						mouse.setSocket(ws);
					}
					keyboard.setSocket(ws);
					__installed = true;
				} else {
					tools.error("Can't resolve HID features:", http.responseText);
					__install_timer = setTimeout(() => hid.installCapture(ws), 1000);
				}
			}
		});
	};

	this.clearCapture = function() {
		if (__install_timer) {
			clearTimeout(__install_timer);
			__install_timer = null;
		}
		if (__installed) {
			mouse.setSocket(null);
			keyboard.setSocket(null);
			__installed = false;
		}
	};
}
