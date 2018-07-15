var atx = new function() {
	this.setLedsState = function(leds) {
		$("atx-power-led").className = (leds.power ? "led-on" : "led-off");
		$("atx-hdd-led").className = (leds.hdd ? "led-busy" : "led-off");
	};

	this.clearLeds = function() {
		atx.setLedsState(false, false);
	};

	this.clickButton = function(el_button) {
		switch (el_button.id) {
			case "atx-power-button":
				var button = "power";
				var confirm_msg = "Are you sure to click the power button?";
				break;
			case "atx-power-button-long":
				var button = "power_long";
				var confirm_msg = "Are you sure to perform the long press of the power button?";
				break;
			case "atx-reset-button":
				var button = "reset";
				var confirm_msg = "Are you sure to reboot the server?";
				break;
			default:
				var button = null;
				var confirm_msg = null;
				break;
		}

		if (button && confirm(confirm_msg)) {
			__setButtonsBusy(true);
			var http = tools.makeRequest("POST", "/kvmd/atx/click?button=" + button, function() {
				if (http.readyState === 4) {
					if (http.status === 409) {
						alert("Performing another ATX operation for other client, please try again later");
					} else if (http.status !== 200) {
						alert("Click error:", http.responseText);
					}
					__setButtonsBusy(false);
				}
			});
		}
	};

	var __setButtonsBusy = function(busy) {
		[
			"atx-power-button",
			"atx-power-button-long",
			"atx-reset-button",
		].forEach(function(name) {
			tools.setButtonBusy(document.getElementById(name), busy);
		});
	};
};
