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
		let buttons_enabled = false;
		if (state) {
			tools.feature.setEnabled($("atx-dropdown"), state.enabled);
			$("atx-power-led").className = (state.busy ? "led-yellow" : (state.leds.power ? "led-green" : "led-gray"));
			$("atx-hdd-led").className = (state.leds.hdd ? "led-red" : "led-gray");
			buttons_enabled = !state.busy;
		} else {
			$("atx-power-led").className = "led-gray";
			$("atx-hdd-led").className = "led-gray";
		}
		for (let id of ["atx-power-button", "atx-power-button-long", "atx-reset-button"]) {
			tools.el.setEnabled($(id), buttons_enabled);
		}
	};

	var __clickAtx = function(button) {
		let click_button = function() {
			tools.httpPost("/api/atx/click", {"button": button}, function(http) {
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
				Are you sure you want to press the <b>${button}</b> button?<br>
				Warning! This could case data loss on the server.
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
