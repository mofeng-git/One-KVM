/*****************************************************************************
#                                                                            #
#    KVMD - The The main Pi-KVM daemon.                                      #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
#                                                                            #
#    This program is free software: you can redistribute it and/or modify    #
#    it under the terms of the GNU General Public License as published by    #
#    the Free Software Foundation, either version 3 of the License, or       #
#    (at your option) any later version.                                     #
#                                                                            #
#    This program is distributed in the hope that it will be useful,         #
#    but WITHOUT ANY WARRANTY; without even the implied warranty of          #
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the           #
#    GNU General Public License for more details.                            #
#                                                                            #
#    You should have received a copy of the GNU General Public License       #
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.  #
#                                                                            #
*****************************************************************************/


function Atx() {
	var self = this;

	/************************************************************************/

	var __init__ = function() {
		$("atx-power-led").title = "Power Led";
		$("atx-hdd-led").title = "Disk Activity Led";

		tools.setOnClick($("atx-power-button"), () => __clickButton("power", "Are you sure to click the power button?"));
		tools.setOnClick($("atx-power-button-long"), () => __clickButton("power_long", "Are you sure to perform the long press of the power button?"));
		tools.setOnClick($("atx-reset-button"), () => __clickButton("reset", "Are you sure to reboot the server?"));
	};

	/************************************************************************/

	self.setState = function(state) {
		$("atx-power-led").className = ((state && state.leds.power) ? "led-green" : "led-gray");
		$("atx-hdd-led").className = ((state && state.leds.hdd) ? "led-red" : "led-gray");

		wm.switchDisabled($("atx-power-button"), (!state || state.busy));
		wm.switchDisabled($("atx-power-button-long"), (!state || state.busy));
		wm.switchDisabled($("atx-reset-button"), (!state || state.busy));
	};

	var __clickButton = function(button, confirm_msg) {
		wm.confirm(confirm_msg).then(function(ok) {
			if (ok) {
				var http = tools.makeRequest("POST", "/kvmd/atx/click?button=" + button, function() {
					if (http.readyState === 4) {
						if (http.status === 409) {
							wm.error("Performing another ATX operation for other client.<br>Please try again later");
						} else if (http.status !== 200) {
							wm.error("Click error:<br>", http.responseText);
						}
					}
				});
			}
		});
	};

	__init__();
}
