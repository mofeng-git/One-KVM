/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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


"use strict";


import {tools, $} from "../tools.js";
import {wm} from "../wm.js";


export function Atx(__recorder) {
	var self = this;

	/************************************************************************/

	var __has_switch = null; // Or true/false
	var __state = null;

	var __init__ = function() {
		$("atx-power-led").title = "Power Led";
		$("atx-hdd-led").title = "Disk Activity Led";

		tools.storage.bindSimpleSwitch($("atx-ask-switch"), "atx.ask", true);

		tools.el.setOnClick($("atx-power-button"), () => __clickAtx("power"));
		tools.el.setOnClick($("atx-power-button-long"), () => __clickAtx("power_long"));
		tools.el.setOnClick($("atx-reset-button"), () => __clickAtx("reset"));
	};

	/************************************************************************/

	self.setState = function(state) {
		if (state) {
			if (!__state) {
				__state = {"leds": {}};
			}
			if (state.enabled !== undefined) {
				__state.enabled = state.enabled;
				tools.feature.setEnabled($("atx-dropdown"), (__state.enabled && !__has_switch));
			}
			if (__state.enabled !== undefined) {
				if (state.busy !== undefined) {
					__updateButtons(!state.busy);
					__state.busy = state.busy;
				}
				if (state.leds !== undefined) {
					__state.leds = state.leds;
				}
				if (state.busy !== undefined || state.leds !== undefined) {
					__updateLeds(__state.leds.power, __state.leds.hdd, __state.busy);
				}
			}
		} else {
			__state = null;
			__updateLeds(false, false, false);
			__updateButtons(false);
		}
	};

	self.setHasSwitch = function(has_switch) {
		__has_switch = has_switch;
		self.setState(__state);
	};

	var __updateLeds = function(power, hdd, busy) {
		$("atx-power-led").className = (busy ? "led-yellow" : (power ? "led-green" : "led-gray"));
		$("atx-hdd-led").className = (hdd ? "led-red" : "led-gray");
	};

	var __updateButtons = function(enabled) {
		for (let id of ["atx-power-button", "atx-power-button-long", "atx-reset-button"]) {
			tools.el.setEnabled($(id), enabled);
		}
	};

	var __clickAtx = function(button) {
		let click_button = function() {
			tools.httpPost("api/atx/click", {"button": button}, function(http) {
				if (http.status === 409) {
					wm.error("Performing another ATX operation for other client.<br>Please try again later.");
				} else if (http.status !== 200) {
					wm.error("Click error", http.responseText);
				}
			});
			__recorder.recordAtxButtonEvent(button);
		};

		if ($("atx-ask-switch").checked) {
			wm.confirm(`
				Are you sure you want to press the <b>${tools.escape(button)}</b> button?<br>
				Warning! This could cause data loss on the server.
			`).then(function(ok) {
				if (ok) {
					click_button();
				}
			});
		} else {
			click_button();
		}
	};

	__init__();
}
