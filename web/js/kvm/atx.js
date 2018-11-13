function Atx() {
	var self = this;

	/********************************************************************************/

	var __init__ = function() {
		$("atx-power-led").title = "Power Led";
		$("atx-hdd-led").title = "Disk Activity Led";

		tools.setOnClick($("atx-power-button"), () => __clickButton("power", null, "Are you sure to click the power button?"));
		tools.setOnClick($("atx-power-button-long"), () => __clickButton("power_long", 15000, "Are you sure to perform the long press of the power button?"));
		tools.setOnClick($("atx-reset-button"), () => __clickButton("reset", null, "Are you sure to reboot the server?"));
	};

	/********************************************************************************/

	self.loadInitialState = function() {
		var http = tools.makeRequest("GET", "/kvmd/atx", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					__setButtonsBusy(JSON.parse(http.responseText).result.busy);
				} else {
					setTimeout(self.loadInitialState, 1000);
				}
			}
		});
	};

	self.setState = function(state) {
		__setButtonsBusy(state.busy);
		$("atx-power-led").className = (state.leds.power ? "led-green" : "led-gray");
		$("atx-hdd-led").className = (state.leds.hdd ? "led-red" : "led-gray");
	};

	self.clearState = function() {
		$("atx-power-led").className = "led-gray";
		$("atx-hdd-led").className = "led-gray";
	};

	var __clickButton = function(button, timeout, confirm_msg) {
		ui.confirm(confirm_msg).then(function(ok) {
			if (ok) {
				var http = tools.makeRequest("POST", "/kvmd/atx/click?button=" + button, function() {
					if (http.readyState === 4) {
						if (http.status === 409) {
							ui.error("Performing another ATX operation for other client.<br>Please try again later");
						} else if (http.status !== 200) {
							ui.error("Click error:<br>", http.responseText);
						}
					}
				}, timeout);
			}
		});
	};

	var __setButtonsBusy = function(busy) {
		$("atx-power-button").disabled = busy;
		$("atx-power-button-long").disabled = busy;
		$("atx-reset-button").disabled = busy;
	};

	__init__();
}
