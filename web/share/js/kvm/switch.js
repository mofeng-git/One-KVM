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


export function Switch() {
	var self = this;

	/************************************************************************/

	var __state = null;
	var __msd_connected = false;

	var __init__ = function() {
		tools.selector.addOption($("switch-edid-selector"), "Default", "default");
		$("switch-edid-selector").onchange = __selectEdid;

		tools.el.setOnClick($("switch-edid-add-button"), __clickAddEdidButton);
		tools.el.setOnClick($("switch-edid-remove-button"), __clickRemoveEdidButton);
		tools.el.setOnClick($("switch-edid-copy-data-button"), __clickCopyEdidDataButton);

		tools.storage.bindSimpleSwitch($("switch-atx-ask-switch"), "switch.atx.ask", true);

		for (let role of ["inactive", "active", "flashing", "beacon", "bootloader"]) {
			let el_brightness = $(`switch-color-${role}-brightness-slider`);
			tools.slider.setParams(el_brightness, 0, 255, 1, 0);
			el_brightness.onchange = $(`switch-color-${role}-input`).onchange = tools.partial(__selectColor, role);
			tools.el.setOnClick($(`switch-color-${role}-default-button`), tools.partial(__clickSetDefaultColorButton, role));
		}
	};

	/************************************************************************/

	self.setMsdConnected = function(connected) {
		__msd_connected = connected;
	};

	self.setState = function(state) {
		if (state) {
			if (!__state) {
				__state = {};
			}
			if (state.model) {
				__state = {};
				__applyModel(state.model);
			}
			if (__state.model) {
				if (state.summary) {
					__applySummary(state.summary);
				}
				if (state.beacons) {
					__applyBeacons(state.beacons);
				}
				if (state.usb) {
					__applyUsb(state.usb);
				}
				if (state.video) {
					__applyVideo(state.video);
				}
				if (state.atx) {
					__applyAtx(state.atx);
				}
				if (state.edids) {
					__applyEdids(state.edids);
				}
				if (state.colors) {
					__applyColors(state.colors);
				}
			}
		} else {
			tools.feature.setEnabled($("switch-dropdown"), false);
			$("switch-chain").innerText = "";
			$("switch-active-port").innerText = "N/A";
			__setPowerLedState($("switch-atx-power-led"), false, false);
			__setLedState($("switch-atx-hdd-led"), "red", false);
			__state = null;
		}
	};

	var __applyColors = function(colors) {
		for (let role in colors) {
			let color = colors[role];
			$(`switch-color-${role}-input`).value = (
				"#"
				+ color.red.toString(16).padStart(2, "0")
				+ color.green.toString(16).padStart(2, "0")
				+ color.blue.toString(16).padStart(2, "0")
			);
			$(`switch-color-${role}-brightness-slider`).value = color.brightness;
		}
		__state.colors = colors;
	};

	var __selectColor = function(role) {
		let el_color = $(`switch-color-${role}-input`);
		let el_brightness = $(`switch-color-${role}-brightness-slider`);
		let color = __state.colors[role];
		let brightness = parseInt(el_brightness.value);
		let rgbx = (
			el_color.value.slice(1)
			+ ":" + brightness.toString(16).padStart(2, "0")
			+ ":" + color.blink_ms.toString(16).padStart(4, "0")
		);
		__sendPost("/api/switch/set_colors", {[role]: rgbx}, function() {
			el_color.value = (
				"#"
				+ color.red.toString(16).padStart(2, "0")
				+ color.green.toString(16).padStart(2, "0")
				+ color.blue.toString(16).padStart(2, "0")
			);
			el_brightness.value = color.brightness;
		});
	};

	var __clickSetDefaultColorButton = function(role) {
		__sendPost("/api/switch/set_colors", {[role]: "default"});
	};

	var __applyEdids = function(edids) {
		let el = $("switch-edid-selector");
		let old_edid_id = el.value;
		el.options.length = 1;
		for (let kv of Object.entries(edids.all)) {
			if (kv[0] !== "default") {
				tools.selector.addOption(el, kv[1].name, kv[0]);
			}
		}
		el.value = (old_edid_id in edids.all ? old_edid_id : "default");

		for (let port in __state.model.ports) {
			let custom = (edids.used[port] !== "default");
			$(`__switch-custom-edid-p${port}`).style.visibility = (custom ? "unset" : "hidden");
		}

		__state.edids = edids;
		__selectEdid();
	};

	var __selectEdid = function() {
		let edid_id = $("switch-edid-selector").value;
		let edid = null;
		try { edid = __state.edids.all[edid_id]; } catch { edid_id = ""; }
		let parsed = (edid ? edid.parsed : null);
		let na = "<i>&lt;Not Available&gt;</i>";
		$("switch-edid-info-mfc-id").innerHTML = (parsed ? tools.escape(parsed.mfc_id) : na);
		$("switch-edid-info-product-id").innerHTML = (parsed ? tools.escape(`0x${parsed.product_id.toString(16).toUpperCase()}`) : na);
		$("switch-edid-info-serial").innerHTML = (parsed ? tools.escape(`0x${parsed.serial.toString(16).toUpperCase()}`) : na);
		$("switch-edid-info-monitor-name").innerHTML = ((parsed && parsed.monitor_name) ? tools.escape(parsed.monitor_name) : na);
		$("switch-edid-info-monitor-serial").innerHTML = ((parsed && parsed.monitor_serial) ? tools.escape(parsed.monitor_serial) : na);
		$("switch-edid-info-audio").innerHTML = (parsed ? (parsed.audio ? "Yes" : "No") : na);
		tools.el.setEnabled($("switch-edid-remove-button"), (edid_id && (edid_id !== "default")));
		tools.el.setEnabled($("switch-edid-copy-data-button"), !!edid_id);
	};

	var __clickAddEdidButton = function() {
		let create_content = function(el_parent, el_ok_button) {
			tools.el.setEnabled(el_ok_button, false);
			el_parent.innerHTML = `
				<table>
					<tr>
						<td>Name:</td>
						<td><input
							type="text" autocomplete="off" id="__switch-edid-new-name-input"
							placeholder="Enter some meaningful name"
							style="width:100%"
						/></td>
					</tr>
					<tr><td colspan="2">HEX data:</td></tr>
					<tr>
						<td colspan="2"><textarea
							id="__switch-edid-new-data-text" placeholder="Like 0123ABCD..."
							style="min-width:350px"
						></textarea><td>
				</table>
			`;
			let el_name = $("__switch-edid-new-name-input");
			let el_data = $("__switch-edid-new-data-text");
			el_name.oninput = el_data.oninput = function() {
				let name = el_name.value.replace(/\s+/g, "");
				let data = el_data.value.replace(/\s+/g, "");
				tools.el.setEnabled(el_ok_button, ((name.length > 0) && /[0-9a-fA-F]{512}/.test(data)));
			};
		};

		wm.modal("Add new EDID", create_content, true, true).then(function(ok) {
			if (ok) {
				let name = $("__switch-edid-new-name-input").value;
				let data = $("__switch-edid-new-data-text").value;
				__sendPost("/api/switch/edids/create", {"name": name, "data": data});
			}
		});
	};

	var __clickRemoveEdidButton = function() {
		let edid_id = $("switch-edid-selector").value;
		if (edid_id && __state && __state.edids) {
			let name = __state.edids.all[edid_id].name;
			let html = "Are you sure to remove this EDID?<br>Ports that used it will change it to the default.";
			wm.confirm(html, name).then(function(ok) {
				if (ok) {
					__sendPost("/api/switch/edids/remove", {"id": edid_id});
				}
			});
		}
	};

	var __clickCopyEdidDataButton = function() {
		let edid_id = $("switch-edid-selector").value;
		if (edid_id && __state && __state.edids) {
			let data = __state.edids.all[edid_id].data;
			data = data.replace(/(.{32})/g, "$1\n");
			wm.copyTextToClipboard(data);
		}
	};

	var __applyUsb = function(usb) {
		for (let port = 0; port < __state.model.ports.length; ++port) {
			if (!__state.usb || __state.usb.links[port] !== usb.links[port]) {
				__setLedState($(`__switch-usb-led-p${port}`), "green", usb.links[port]);
			}
		}
		__state.usb = usb;
	};

	var __applyVideo = function(video) {
		for (let port = 0; port < __state.model.ports.length; ++port) {
			if (!__state.video || __state.video.links[port] !== video.links[port]) {
				__setLedState($(`__switch-video-led-p${port}`), "green", video.links[port]);
			}
		}
		__state.video = video;
	};

	var __applyAtx = function(atx) {
		for (let port = 0; port < __state.model.ports.length; ++port) {
			let busy = atx.busy[port];
			if (!__state.atx || __state.atx.leds.power[port] !== atx.leds.power[port] || __state.atx.busy[port] !== busy) {
				let power = atx.leds.power[port];
				__setPowerLedState($(`__switch-atx-power-led-p${port}`), power, busy);
				if (port === __state.summary.active_port) {
					// summary есть всегда, если есть model, и atx обновляется последним в setState()
					__setPowerLedState($("switch-atx-power-led"), power, busy);
				}
			}
			if (!__state.atx || __state.atx.leds.hdd[port] !== atx.leds.hdd[port]) {
				let hdd = atx.leds.hdd[port];
				__setLedState($(`__switch-atx-hdd-led-p${port}`), "red", hdd);
				if (port === __state.summary.active_port) {
					__setLedState($("switch-atx-hdd-led"), "red", hdd);
				}
			}
			if (!__state.atx || __state.atx.busy[port] !== busy) {
				tools.el.setEnabled($(`__switch-atx-power-button-p${port}`), !busy);
				tools.el.setEnabled($(`__switch-atx-power-long-button-p${port}`), !busy);
				tools.el.setEnabled($(`__switch-atx-reset-button-p${port}`), !busy);
			}
		}
		__state.atx = atx;
	};

	var __applyBeacons = function(beacons) {
		for (let unit = 0; unit < __state.model.units.length; ++unit) {
			if (!__state.beacons || __state.beacons.uplinks[unit] !== beacons.uplinks[unit]) {
				__setLedState($(`__switch-beacon-led-u${unit}`), "green", beacons.uplinks[unit]);
			}
			if (!__state.beacons || __state.beacons.downlinks[unit] !== beacons.downlinks[unit]) {
				__setLedState($(`__switch-beacon-led-d${unit}`), "green", beacons.downlinks[unit]);
			}
		}
		for (let port = 0; port < __state.model.ports.length; ++port) {
			if (!__state.beacons || __state.beacons.ports[port] !== beacons.ports[port]) {
				__setLedState($(`__switch-beacon-led-p${port}`), "green", beacons.ports[port]);
			}
		}
		__state.beacons = beacons;
	};

	var __applySummary = function(summary) {
		let active = summary.active_port;
		if (!__state.summary || __state.summary.active_port !== active) {
			if (active < 0 || active >= __state.model.ports.length) {
				$("switch-active-port").innerText = "N/A";
			} else {
				$("switch-active-port").innerText = "p" + __formatPort(__state.model, active);
			}
			for (let port = 0; port < __state.model.ports.length; ++port) {
				__setLedState($(`__switch-port-led-p${port}`), "green", (port === active));
			}
		}
		if (__state.atx) {
			// Синхронизация светодиодов ATX при смене порта
			let power = false;
			let busy = false;
			let hdd = false;
			if (active >= 0 && active < __state.model.ports.length) {
				power = __state.atx.leds.power[active];
				hdd = __state.atx.leds.hdd[active];
				busy = __state.atx.busy[active];
			}
			__setPowerLedState($("switch-atx-power-led"), power, busy);
			__setLedState($("switch-atx-hdd-led"), "red", hdd);
		}
		__state.summary = summary;
	};

	var __applyModel = function(model) {
		tools.feature.setEnabled($("switch-dropdown"), model.ports.length);

		let content = "";
		let unit = -1;
		for (let port = 0; port < model.ports.length; ++port) {
			let pa = model.ports[port]; // pa == port attrs
			if (unit !== pa.unit) {
				unit = pa.unit;
				content += `${unit > 0 ? "<tr><td colspan=100><hr></td></tr>" : ""}
					<tr>
						<td></td><td></td><td></td>
						<td class="value">Unit: ${unit + 1}</td>
						<td></td>
						<td colspan=100>
							<div class="buttons-row">
								<button id="__switch-beacon-button-u${unit}" class="small" title="Toggle uplink Beacon Led">
									<img id="__switch-beacon-led-u${unit}" class="inline-lamp led-gray" src="/share/svg/led-beacon.svg"/>
									Uplink
								</button>
								<button id="__switch-beacon-button-d${unit}" class="small" title="Toggle downlink Beacon Led">
									<img id="__switch-beacon-led-d${unit}" class="inline-lamp led-gray" src="/share/svg/led-beacon.svg"/>
									Downlink
								</button>
							</div>
						</td>
					</tr>
					<tr><td colspan=100><hr></td></tr>
				`;
			}
			content += `
				<tr>
					<td>Port:</td>
					<td class="value">${__formatPort(model, port)}</td>
					<td>&nbsp;&nbsp;</td>
					<td>
						<div class="buttons-row">
							<button id="__switch-port-button-p${port}" title="Activate this port">
								<img id="__switch-port-led-p${port}" class="inline-lamp led-gray" src="/share/svg/led-circle.svg"/>
							</button>
							<button id="__switch-params-button-p${port}" title="Configure this port">
								<img id="__switch-params-led-p${port}" class="inline-lamp led-gray" src="/share/svg/led-gear.svg"/>
							</button>
						</div>
					</td>
					<td>
						<span
							id="__switch-custom-edid-p${port}" style="visibility:hidden"
							title="A non-default EDID is used on this port"
						>
							&#9913;
						</span>
						&nbsp;&nbsp;&nbsp;&nbsp;
						${pa.name.length > 0 ? tools.escape(pa.name) : ("Host " + (port + 1))}
						&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
					</td>
					<td style="font-size:1em">
						<button id="__switch-beacon-button-p${port}" class="small" title="Toggle Beacon Led on this port">
							<img id="__switch-beacon-led-p${port}" class="inline-lamp led-gray" src="/share/svg/led-beacon.svg"/>
						</button>
					</td>
					<td>
						<img id="__switch-video-led-p${port}" class="inline-lamp led-gray" src="/share/svg/led-video.svg" title="Video Link"/>
						<img id="__switch-usb-led-p${port}" class="inline-lamp led-gray" src="/share/svg/led-usb.svg" title="USB Link"/>
						<img id="__switch-atx-power-led-p${port}" class="inline-lamp led-gray" src="/share/svg/led-atx-power.svg" title="Power Led"/>
						<img id="__switch-atx-hdd-led-p${port}" class="inline-lamp led-gray" src="/share/svg/led-atx-hdd.svg" title="HDD Led"/>
					</td>
					<td>
						<div class="buttons-row">
							<button id="__switch-atx-power-button-p${port}" class="small">Power <sup><i>short</i></sup></button>
							<button id="__switch-atx-power-long-button-p${port}" class="small"><sup><i>long</i></sup></button>
							<button id="__switch-atx-reset-button-p${port}" class="small">Reset</button>
						</div>
					</td>
				</tr>
			`;
		}
		$("switch-chain").innerHTML = content;

		if (model.units.length > 0) {
			tools.hidden.setVisible($("switch-message-update"), (model.firmware.version > model.units[0].firmware.version));
		}

		for (let unit = 0; unit < model.units.length; ++unit) {
			tools.el.setOnClick($(`__switch-beacon-button-u${unit}`), tools.partial(__switchUplinkBeacon, unit));
			tools.el.setOnClick($(`__switch-beacon-button-d${unit}`), tools.partial(__switchDownlinkBeacon, unit));
		}

		for (let port = 0; port < model.ports.length; ++port) {
			tools.el.setOnClick($(`__switch-port-button-p${port}`), tools.partial(__switchActivePort, port));
			tools.el.setOnClick($(`__switch-params-button-p${port}`), tools.partial(__showParamsDialog, port));
			tools.el.setOnClick($(`__switch-beacon-button-p${port}`), tools.partial(__switchPortBeacon, port));
			tools.el.setOnClick($(`__switch-atx-power-button-p${port}`), tools.partial(__atxClick, port, "power"));
			tools.el.setOnClick($(`__switch-atx-power-long-button-p${port}`), tools.partial(__atxClick, port, "power_long"));
			tools.el.setOnClick($(`__switch-atx-reset-button-p${port}`), tools.partial(__atxClick, port, "reset"));
		}

		__setPowerLedState($("switch-atx-power-led"), false, false);
		__setLedState($("switch-atx-hdd-led"), "red", false);

		__state.model = model;
	};

	var __showParamsDialog = function(port) {
		if (!__state || !__state.model || !__state.edids) {
			return;
		}

		let model = __state.model;
		let edids = __state.edids;

		let atx_actions = {
			"power": "ATX power click",
			"power_long": "Power long",
			"reset": "Reset click",
		};

		let add_edid_option = function(el, attrs, id) {
			tools.selector.addOption(el, attrs.name, id, (edids.used[port] === id));
			if (attrs.parsed !== null) {
				let parsed = attrs.parsed;
				let text = "\xA0\xA0\xA0\xA0\xA0\u2570 ";
				text += (parsed.monitor_name !== null ? parsed.monitor_name : parsed.mfc_id);
				text += (parsed.audio ? "; +Audio" : "; -Audio");
				tools.selector.addComment(el, text);
			}
		};

		let create_content = function(el_parent) {
			let html = `
				<table>
					<tr>
						<td>Port name:</td>
						<td><input
							type="text" autocomplete="off" id="__switch-port-name-input"
							value="${tools.escape(model.ports[port].name)}" placeholder="Host ${port + 1}"
							style="width:100%"
						/></td>
					</tr>
					<tr>
						<td>EDID:</td>
						<td><select id="__switch-port-edid-selector" style="width: 100%"></select></td>
					</tr>
				</table>
				<hr>
				<table>
			`;
			for (let kv of Object.entries(atx_actions)) {
				html += `
					<tr>
						<td style="white-space: nowrap">${tools.escape(kv[1])}:</td>
						<td style="width: 100%"><input type="range" id="__switch-port-atx-click-${kv[0]}-delay-slider"/></td>
						<td id="__switch-port-atx-click-${kv[0]}-delay-value"></td>
						<td>&nbsp;&nbsp;&nbsp;</td>
						<td><button
							id="__switch-port-atx-click-${kv[0]}-delay-default-button"
							class="small" title="Reset default"
						>&#8635;</button></td>
					</tr>
				`;
			}
			html += "</table>";
			el_parent.innerHTML = html;

			let el_selector = $("__switch-port-edid-selector");
			add_edid_option(el_selector, edids.all["default"], "default");
			for (let kv of Object.entries(edids.all)) {
				if (kv[0] !== "default") {
					tools.selector.addSeparator(el_selector, 20);
					add_edid_option(el_selector, kv[1], kv[0]);
				}
			}

			for (let action of Object.keys(atx_actions)) {
				let limits = model.limits.atx.click_delays[action];
				let el_slider = $(`__switch-port-atx-click-${action}-delay-slider`);
				let display_value = tools.partial(function(action, value) {
					$(`__switch-port-atx-click-${action}-delay-value`).innerText = `${value.toFixed(1)}`;
				}, action);
				let reset_default = tools.partial(function(el_slider, limits) {
					tools.slider.setValue(el_slider, limits["default"]);
				}, el_slider, limits);
				tools.slider.setParams(el_slider, limits.min, limits.max, 0.5, model.ports[port].atx.click_delays[action], display_value);
				tools.el.setOnClick($(`__switch-port-atx-click-${action}-delay-default-button`), reset_default);
			}
		};

		wm.modal(`Port ${__formatPort(__state.model, port)} settings`, create_content, true, true).then(function(ok) {
			if (ok) {
				let params = {
					"port": port,
					"edid_id": $("__switch-port-edid-selector").value,
					"name": $("__switch-port-name-input").value,
				};
				for (let action of Object.keys(atx_actions)) {
					params[`atx_click_${action}_delay`] = tools.slider.getValue($(`__switch-port-atx-click-${action}-delay-slider`));
				};
				__sendPost("/api/switch/set_port_params", params);
			}
		});
	};

	var __formatPort = function(model, port) {
		if (model.units.length > 1) {
			return `${model.ports[port].unit + 1}.${model.ports[port].channel + 1}`;
		} else {
			return `${port + 1}`;
		}
	};

	var __setLedState = function(el, color, on) {
		el.classList.toggle(`led-${color}`, on);
		el.classList.toggle("led-gray", !on);
	};

	var __setPowerLedState = function(el, power, busy) {
		el.classList.toggle("led-green", (power && !busy));
		el.classList.toggle("led-yellow", busy);
		el.classList.toggle("led-gray", !(power || busy));
	};

	var __switchActivePort = function(port) {
		if (__msd_connected) {
			wm.error(`
				Oops! Before port switching, please disconnect an active Mass Storage Drive image first.
				Otherwise, it will break a current USB operation (OS installation, Live CD, or whatever).
			`);
		} else {
			__sendPost("/api/switch/set_active", {"port": port});
		}
	};

	var __switchUplinkBeacon = function(unit) {
		let state = false;
		try { state = !__state.beacons.uplinks[unit]; } catch {}; // eslint-disable-line no-empty
		__sendPost("/api/switch/set_beacon", {"uplink": unit, "state": state});
	};

	var __switchDownlinkBeacon = function(unit) {
		let state = false;
		try { state = !__state.beacons.downlinks[unit]; } catch {}; // eslint-disable-line no-empty
		__sendPost("/api/switch/set_beacon", {"downlink": unit, "state": state});
	};

	var __switchPortBeacon = function(port) {
		let state = false;
		try { state = !__state.beacons.ports[port]; } catch {}; // eslint-disable-line no-empty
		__sendPost("/api/switch/set_beacon", {"port": port, "state": state});
	};

	var __atxClick = function(port, button) {
		let click_button = function() {
			__sendPost("/api/switch/atx/click", {"port": port, "button": button});
		};
		if ($("switch-atx-ask-switch").checked) {
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

	var __sendPost = function(url, params, error_callback=null) {
		tools.httpPost(url, params, function(http) {
			if (http.status !== 200) {
				if (error_callback) {
					error_callback();
				}
				wm.error("Switch error", http.responseText);
			}
		});
	};

	__init__();
}
