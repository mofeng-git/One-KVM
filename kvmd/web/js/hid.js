var hid = new function() {
	var __install_timer = null;

	this.installCapture = function(ws) {
		var http = tools.makeRequest("GET", "/kvmd/hid", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					features = JSON.parse(http.responseText).result.features;
					if (features.mouse) {
						mouse.installCapture(ws);
					}
					keyboard.installCapture(ws);
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
		mouse.clearCapture();
		keyboard.clearCapture();
	};
}
