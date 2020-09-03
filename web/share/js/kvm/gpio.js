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


import {tools, $, $$$} from "../tools.js";
import {wm} from "../wm.js";


export function Gpio() {
	var self = this;

	/************************************************************************/

	var __state = null;

	/************************************************************************/

	self.setState = function(state) {
		if (state) {
			for (let channel in state.inputs) {
				let el = $(`gpio-led-${channel}`);
				if (el) {
					__setLedState(el, state.inputs[channel].state);
				}
			}
			for (let channel in state.outputs) {
				for (let type of ["switch", "button"]) {
					let el = $(`gpio-${type}-${channel}`);
					if (el) {
						wm.switchEnabled(el, !state.outputs[channel].busy);
					}
				}
			}
		} else {
			for (let el of $$$(".gpio-led")) {
				__setLedState(el, false);
			}
			for (let selector of [".gpio-switch", ".gpio-button"]) {
				for (let el of $$$(selector)) {
					wm.switchEnabled(el, false);
				}
			}
		}
		__state = state;
	};

	self.setModel = function(model) {
		tools.featureSetEnabled($("gpio-dropdown"), model.view.table.length);
		if (model.view.table.length) {
			$("gpio-menu-button").innerHTML = `${model.view.header.title} &#8628;`;
		}

		let switches = [];
		let buttons = [];
		let content = "<table class=\"kv\">";
		for (let row of model.view.table) {
			if (row === null) {
				content += "</table><hr><table class=\"kv\">";
			} else {
				content += "<tr>";
				for (let item of row) {
					if (item.type === "output") {
						item.scheme = model.scheme.outputs[item.channel];
					}
					content += `<td>${__createItem(item, switches, buttons)}</td>`;
				}
				content += "</tr>";
			}
		}
		content += "</table>";
		$("gpio-menu").innerHTML = content;

		for (let channel of switches) {
			tools.setOnClick($(`gpio-switch-${channel}`), () => __switchChannel(channel));
		}
		for (let channel of buttons) {
			tools.setOnClick($(`gpio-button-${channel}`), () => __pulseChannel(channel));
		}

		self.setState(__state);
	};

	var __createItem = function(item, switches, buttons) {
		if (item.type === "label") {
			return item.text;
		} else if (item.type === "input") {
			return `<img id="gpio-led-${item.channel}" class="gpio-led inline-lamp-big led-gray" src="/share/svg/led-square.svg" />`;
		} else if (item.type === "output") {
			let controls = [];
			if (item.scheme["switch"]) {
				switches.push(item.channel);
				controls.push(`
					<td><div class="switch-box">
						<input disabled type="checkbox" id="gpio-switch-${item.channel}" class="gpio-switch" />
						<label for="gpio-switch-${item.channel}">
							<span class="switch-inner"></span>
							<span class="switch"></span>
						</label>
					</div></td>
				`);
			}
			if (item.scheme.pulse.delay) {
				buttons.push(item.channel);
				controls.push(`<td><button disabled id="gpio-button-${item.channel}" class="gpio-button">${item.text}</button></td>`);
			}
			return `<table><tr>${controls.join("<td>&nbsp;&nbsp;&nbsp;</td>")}</tr></table>`;
		} else {
			return "";
		}
	};

	var __setLedState = function(el, state) {
		if (state) {
			el.classList.add("led-green");
			el.classList.remove("led-gray");
		} else {
			el.classList.add("led-gray");
			el.classList.remove("led-green");
		}
	};

	var __switchChannel = function(channel) {
		let to = ($(`gpio-switch-${channel}`).checked ? "1" : "0");
		__sendPost(`/api/gpio/switch?channel=${channel}&state=${to}`);
	};

	var __pulseChannel = function(channel) {
		__sendPost(`/api/gpio/pulse?channel=${channel}`);
	};

	var __sendPost = function(url) {
		let http = tools.makeRequest("POST", url, function() {
			if (http.readyState === 4) {
				if (http.status === 409) {
					wm.error("Performing another operation for this GPIO channel.<br>Please try again later");
				} else if (http.status !== 200) {
					wm.error("GPIO error:<br>", http.responseText);
				}
			}
		});
	};
}
