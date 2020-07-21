/*****************************************************************************
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
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


"use strict";


import {tools, $} from "../tools.js";
import {wm} from "../wm.js";


export function WakeOnLan() {
	var self = this;

	/************************************************************************/

	var __target = {};

	var __init__ = function() {
		tools.setOnClick($("wol-wakeup-button"), __clickWakeupButton);
	};

	/************************************************************************/

	self.setState = function(state) {
		if (state) {
			tools.featureSetEnabled($("wol"), state.enabled);
			__target = state.target;
		}
		wm.switchEnabled($("wol-wakeup-button"), (state && state.enabled));
	};

	var __clickWakeupButton = function() {
		let msg = `
			Are you sure to send Wake-on-LAN packet to the server?<br>
			Target: <b>${__target.mac}</b> (${__target.ip}:${__target.port})?
		`;
		wm.confirm(msg).then(function(ok) {
			if (ok) {
				let http = tools.makeRequest("POST", "/api/wol/wakeup", function() {
					if (http.readyState === 4) {
						if (http.status !== 200) {
							wm.error("Wakeup error:<br>", http.responseText);
						}
					}
				});
			}
		});
	};

	__init__();
}
