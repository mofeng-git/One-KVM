function Atx() {
	var self = this;

	/********************************************************************************/

	var __init__ = function() {
		$("atx-power-led").title = "Power Led";
		$("atx-hdd-led").title = "Disk Activity Led";

		tools.setOnClick($("atx-power-button"), () => __clickButton("power", "Are you sure to click the power button?"));
		tools.setOnClick($("atx-power-button-long"), () => __clickButton("power_long", "Are you sure to perform the long press of the power button?", 15000));
		tools.setOnClick($("atx-reset-button"), () => __clickButton("reset", "Are you sure to reboot the server?"));
	};

	/********************************************************************************/

	self.setState = function(state) {
		$("atx-power-led").className = (state.leds.power ? "led-green" : "led-gray");
		$("atx-hdd-led").className = (state.leds.hdd ? "led-red" : "led-gray");

		$("atx-power-button").disabled = state.busy;
		$("atx-power-button-long").disabled = state.busy;
		$("atx-reset-button").disabled = state.busy;
	};

	self.clearState = function() {
		$("atx-power-led").className = "led-gray";
		$("atx-hdd-led").className = "led-gray";
	};

	var __clickButton = function(button, confirm_msg, timeout=null) {
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

	__init__();
}
