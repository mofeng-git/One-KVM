var atx = new function() {
	this.loadInitialState = function() {
		var http = tools.makeRequest("GET", "/kvmd/atx", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					atx.setButtonsBusy(JSON.parse(http.responseText).result.busy);
				} else {
					setTimeout(atx.loadInitialState, 1000);
				}
			}
		});
	};

	this.setState = function(state) {
		atx.setButtonsBusy(state.busy);
		$("atx-power-led").className = (state.leds.power ? "led-on" : "led-off");
		$("atx-hdd-led").className = (state.leds.hdd ? "led-hdd-busy" : "led-off");
	};

	this.clearState = function() {
		[
			"atx-power-led",
			"atx-hdd-led",
		].forEach(function(name) {
			$(name).className = "led-off";
		});
	};

	this.clickButton = function(el) {
		var button = null;
		var confirm_msg = null;
		var timeout = null;

		switch (el.id) {
			case "atx-power-button":
				var button = "power";
				var confirm_msg = "Are you sure to click the power button?";
				break;
			case "atx-power-button-long":
				var button = "power_long";
				var confirm_msg = "Are you sure to perform the long press of the power button?";
				var timeout = 15000;
				break;
			case "atx-reset-button":
				var button = "reset";
				var confirm_msg = "Are you sure to reboot the server?";
				break;
		}

		if (button && confirm(confirm_msg)) {
			// atx.setButtonsBusy(true);
			var http = tools.makeRequest("POST", "/kvmd/atx/click?button=" + button, function() {
				if (http.readyState === 4) {
					if (http.status === 409) {
						alert("Performing another ATX operation for other client, please try again later");
					} else if (http.status !== 200) {
						alert("Click error:", http.responseText);
					}
					// atx.setButtonsBusy(false);
				}
			}, timeout);
		}
	};

	this.setButtonsBusy = function(busy) {
		[
			"atx-power-button",
			"atx-power-button-long",
			"atx-reset-button",
		].forEach(function(name) {
			tools.setButtonBusy(document.getElementById(name), busy);
		});
	};
};
