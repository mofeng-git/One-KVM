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


import {ROOT_PREFIX} from "../vars.js";
import {tools, $, $$} from "../tools.js";
import {wm} from "../wm.js";


export function Gpio(__recorder) {
	var self = this;

	/************************************************************************/

	var __has_model = false;

	/************************************************************************/

	self.setState = function(state) {
		if (state) {
			if (state.model !== undefined) {
				__has_model = true;
				__updateModel(state.model);
			}
			if (__has_model && state.state !== undefined) {
				if (state.state.inputs !== undefined) {
					__updateInputs(state.state.inputs);
				}
				if (state.state.outputs !== undefined) {
					__updateOutputs(state.state.outputs);
				}
			}
		} else {
			__has_model = false;
			for (let el of $$("__gpio-led")) {
				__setLedState(el, false);
			}
			for (let selector of ["__gpio-switch", "__gpio-button"]) {
				for (let el of $$(selector)) {
					tools.el.setEnabled(el, false);
				}
			}
		}
	};

	var __updateInputs = function(inputs) {
		for (let ch in inputs) {
			for (let el of $$(`__gpio-led-${ch}`)) {
				__setLedState(el, inputs[ch].state);
			}
		}
	};

	var __updateOutputs = function(outputs) {
		for (let ch in outputs) {
			for (let type of ["switch", "button"]) {
				for (let el of $$(`__gpio-${type}-${ch}`)) {
					tools.el.setEnabled(el, (outputs[ch].online && !outputs[ch].busy));
				}
			}
			for (let el of $$(`__gpio-switch-${ch}`)) {
				el.checked = outputs[ch].state;
			}
		}
	};

	var __updateModel = function(model) {
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

		let html = "<table class=\"kv\">";
		for (let row of model.view.table) {
			if (row === null) {
				html += "</table><hr><table class=\"kv\">";
			} else {
				html += "<tr>";
				for (let item of row) {
					if (item.type === "output") {
						item.scheme = model.scheme.outputs[item.channel];
					}
					html += `<td align="center">${__createItem(item)}</td>`;
				}
				html += "</tr>";
			}
		}
		html += "</table>";
		$("gpio-menu").innerHTML = html;

		for (let ch in model.scheme.outputs) {
			for (let el of $$(`__gpio-switch-${ch}`)) {
				tools.el.setOnClick(el, tools.partial(__switchChannel, el));
			}
			for (let el of $$(`__gpio-button-${ch}`)) {
				tools.el.setOnClick(el, tools.partial(__pulseChannel, el));
			}
		}

		tools.feature.setEnabled($("v3-usb-breaker"), ("__v3_usb_breaker__" in model.scheme.outputs));
		tools.feature.setEnabled($("v4-locator"), ("__v4_locator__" in model.scheme.outputs));
		tools.feature.setEnabled($("system-tool-wol"), ("__wol__" in model.scheme.outputs));
	};

	var __createItem = function(item) {
		if (item.type === "label") {
			return item.text;
		} else if (item.type === "input") {
			return `
				<img
					class="__gpio-led __gpio-led-${item.channel} inline-lamp-big led-gray"
					src="${ROOT_PREFIX}share/svg/led-circle.svg"
					data-color="${item.color}"
				/>
			`;
		} else if (item.type === "output") {
			let controls = [];
			let confirm = (item.confirm ? "Are you sure you want to perform this action?" : "");
			if (item.scheme["switch"]) {
				let id = tools.makeId();
				controls.push(`
					<td><div class="switch-box">
						<input
							disabled
							type="checkbox"
							id="__gpio-switch-${id}"
							class="__gpio-switch __gpio-switch-${item.channel}"
							data-channel="${item.channel}"
							data-confirm="${confirm}"
						/>
						<label for="__gpio-switch-${id}">
							<span class="switch-inner"></span>
							<span class="switch"></span>
						</label>
					</div></td>
				`);
			}
			if (item.scheme.pulse.delay) {
				controls.push(`
					<td><button
						disabled
						class="__gpio-button __gpio-button-${item.channel}"
						${item.hide ? "data-force-hide-menu" : ""}
						data-channel="${item.channel}"
						data-confirm="${confirm}"
					>
						${(item.hide ? "&bull; " : "") + item.text}
					</button></td>
				`);
			}
			return `<table><tr>${controls.join("<td>&nbsp;&nbsp;&nbsp;</td>")}</tr></table>`;
		} else {
			return "";
		}
	};

	var __setLedState = function(el, on) {
		let color = el.getAttribute("data-color");
		if (on) {
			el.classList.add(`led-${color}`);
			el.classList.remove("led-gray");
		} else {
			el.classList.add("led-gray");
			el.classList.remove(`led-${color}`);
		}
	};

	var __switchChannel = function(el) {
		let ch = el.getAttribute("data-channel");
		let confirm = el.getAttribute("data-confirm");
		let to = (el.checked ? "1" : "0");
		if (to === "0" && el.hasAttribute("data-confirm-off")) {
			confirm = el.getAttribute("data-confirm-off");
		}
		let act = () => {
			__sendPost("api/gpio/switch", {"channel": ch, "state": to});
			__recorder.recordGpioSwitchEvent(ch, to);
		};
		if (confirm) {
			wm.confirm(tools.escape(confirm)).then(function(ok) {
				if (ok) {
					act();
				}
			});
		} else {
			act();
		}
	};

	var __pulseChannel = function(el) {
		let ch = el.getAttribute("data-channel");
		let confirm = el.getAttribute("data-confirm");
		let act = () => {
			__sendPost("api/gpio/pulse", {"channel": ch});
			__recorder.recordGpioPulseEvent(ch);
		};
		if (confirm) {
			wm.confirm(tools.escape(confirm)).then(function(ok) {
				if (ok) {
					act();
				}
			});
		} else {
			act();
		}
	};

	var __sendPost = function(url, params) {
		tools.httpPost(url, params, function(http) {
			if (http.status === 409) {
				wm.error("Performing another operation for this GPIO channel.<br>Please try again later.");
			} else if (http.status !== 200) {
				wm.error("GPIO error", http.responseText);
			}
		});
	};
}
