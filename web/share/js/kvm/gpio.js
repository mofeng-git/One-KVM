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


import {tools, $, $$$} from "../tools.js";
import {wm} from "../wm.js";


export function Gpio(__recorder) {
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
						tools.el.setEnabled(el, state.outputs[channel].online && !state.outputs[channel].busy);
					}
				}
				let el = $(`gpio-switch-${channel}`);
				if (el) {
					el.checked = state.outputs[channel].state;
				}
			}
		} else {
			for (let el of $$$(".gpio-led")) {
				__setLedState(el, false);
			}
			for (let selector of [".gpio-switch", ".gpio-button"]) {
				for (let el of $$$(selector)) {
					tools.el.setEnabled(el, false);
				}
			}
		}
		__state = state;
	};

	self.setModel = function(model) {
		tools.feature.setEnabled($("gpio-dropdown"), model.view.table.length);
		if (model.view.table.length) {
			let title = [];
			let last_is_label = false;
			for (let item of model.view.header.title) {
				if (last_is_label && item.type === "label") {
					title.push("<span></span>");
				}
				last_is_label = (item.type === "label");
				title.push(__createItem(item));
			}
			$("gpio-menu-button").innerHTML = title.join(" ");
		}

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
					content += `<td align="center">${__createItem(item)}</td>`;
				}
				content += "</tr>";
			}
		}
		content += "</table>";
		$("gpio-menu").innerHTML = content;

		for (let channel in model.scheme.outputs) {
			let el = $(`gpio-switch-${channel}`);
			if (el) {
				tools.el.setOnClick(el, __createAction(el, __switchChannel));
			}
			el = $(`gpio-button-${channel}`);
			if (el) {
				tools.el.setOnClick(el, __createAction(el, __pulseChannel));
			}
		}

		tools.feature.setEnabled($("v3-usb-breaker"), ("__v3_usb_breaker__" in model.scheme.outputs));
		tools.feature.setEnabled($("v4-locator"), ("__v4_locator__" in model.scheme.outputs));
		tools.feature.setEnabled($("system-tool-wol"), ("__wol__" in model.scheme.outputs));

		self.setState(__state);
	};

	var __createAction = function(el, action) {
		return () => action(el);
	};

	var __createItem = function(item) {
		if (item.type === "label") {
			return item.text;
		} else if (item.type === "input") {
			return `
				<img id="gpio-led-${item.channel}" class="gpio-led inline-lamp-big led-gray"
				src="/share/svg/led-circle.svg" data-color="${item.color}" />
			`;
		} else if (item.type === "output") {
			let controls = [];
			let confirm = (item.confirm ? "Are you sure you want to perform this action?" : "");
			if (item.scheme["switch"]) {
				controls.push(`
					<td><div class="switch-box">
						<input disabled type="checkbox" id="gpio-switch-${item.channel}" class="gpio-switch"
						data-channel="${item.channel}" data-confirm="${confirm}" />
						<label for="gpio-switch-${item.channel}">
							<span class="switch-inner"></span>
							<span class="switch"></span>
						</label>
					</div></td>
				`);
			}
			if (item.scheme.pulse.delay) {
				controls.push(`
					<td><button disabled id="gpio-button-${item.channel}" class="gpio-button"
					data-channel="${item.channel}" data-confirm="${confirm}">${item.text}</button></td>
				`);
			}
			return `<table><tr>${controls.join("<td>&nbsp;&nbsp;&nbsp;</td>")}</tr></table>`;
		} else {
			return "";
		}
	};

	var __setLedState = function(el, state) {
		let color = el.getAttribute("data-color");
		if (state) {
			el.classList.add(`led-${color}`);
			el.classList.remove("led-gray");
		} else {
			el.classList.add("led-gray");
			el.classList.remove(`led-${color}`);
		}
	};

	var __switchChannel = function(el) {
		let channel = el.getAttribute("data-channel");
		let confirm = el.getAttribute("data-confirm");
		let to = ($(`gpio-switch-${channel}`).checked ? "1" : "0");
		if (to === "0" && el.hasAttribute("data-confirm-off")) {
			confirm = el.getAttribute("data-confirm-off");
		}
		let act = () => {
			__sendPost(`/api/gpio/switch?channel=${channel}&state=${to}`);
			__recorder.recordGpioSwitchEvent(channel, to);
		};
		if (confirm) {
			wm.confirm(confirm).then(function(ok) {
				if (ok) {
					act();
				} else {
					self.setState(__state); // Switch back
				}
			});
		} else {
			act();
		}
	};

	var __pulseChannel = function(el) {
		let channel = el.getAttribute("data-channel");
		let confirm = el.getAttribute("data-confirm");
		let act = () => {
			__sendPost(`/api/gpio/pulse?channel=${channel}`);
			__recorder.recordGpioPulseEvent(channel);
		};
		if (confirm) {
			wm.confirm(confirm).then(function(ok) { if (ok) act(); });
		} else {
			act();
		}
	};

	var __sendPost = function(url) {
		tools.httpPost(url, function(http) {
			if (http.status === 409) {
				wm.error("Performing another operation for this GPIO channel.<br>Please try again later");
			} else if (http.status !== 200) {
				wm.error("GPIO error:<br>", http.responseText);
			}
		});
	};
}
