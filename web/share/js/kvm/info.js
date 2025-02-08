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
import {tools, $} from "../tools.js";


export function Info() {
	var self = this;

	/************************************************************************/

	var __health_state = null;
	var __fan_state = null;

	var __init__ = function() {
	};

	/************************************************************************/

	self.setState = function(state) {
		for (let key of Object.keys(state)) {
			switch (key) {
				case "meta": __setStateMeta(state.meta); break;
				case "health": __setStateHealth(state.health); break;
				case "fan": __setStateFan(state.fan); break;
				case "system": __setStateSystem(state.system); break;
				case "extras": __setStateExtras(state.extras); break;
			}
		}
	};

	var __setStateMeta = function(state) {
		if (state !== null) {
			$("kvmd-meta-json").innerText = JSON.stringify(state, undefined, 4);

			if (state.server && state.server.host) {
				$("kvmd-meta-server-host").innerText = `Server: ${state.server.host}`;
				document.title = `PiKVM Session: ${state.server.host}`;
			} else {
				$("kvmd-meta-server-host").innerText = "";
				document.title = "PiKVM Session";
			}

			for (let place of ["left", "right"]) {
				if (state.tips && state.tips[place]) {
					$(`kvmd-meta-tips-${place}`).innerText = state.tips[place];
				}
			}

			// Don't use this option, it may be removed in any time
			if (state.web && state.web.confirm_session_exit === false) {
				window.onbeforeunload = null; // See main.js
			}
		}
	};

	var __setStateHealth = function(state) {
		if (state.throttling !== null) {
			let flags = state.throttling.parsed_flags;
			let ignore_past = state.throttling.ignore_past;
			let undervoltage = (flags.undervoltage.now || (flags.undervoltage.past && !ignore_past));
			let freq_capped = (flags.freq_capped.now || (flags.freq_capped.past && !ignore_past));

			tools.hidden.setVisible($("hw-health-dropdown"), (undervoltage || freq_capped));
			$("hw-health-undervoltage-led").className = (undervoltage ? (flags.undervoltage.now ? "led-red" : "led-yellow") : "hidden");
			$("hw-health-overheating-led").className = (freq_capped ? (flags.freq_capped.now ? "led-red" : "led-yellow") : "hidden");
			tools.hidden.setVisible($("hw-health-message-undervoltage"), undervoltage);
			tools.hidden.setVisible($("hw-health-message-overheating"), freq_capped);
		}
		__health_state = state;
		__renderAboutHardware();
	};

	var __setStateFan = function(state) {
		let failed = false;
		let failed_past = false;
		if (state.monitored) {
			if (state.state === null) {
				failed = true;
			} else {
				if (!state.state.fan.ok) {
					failed = true;
				} else if (state.state.fan.last_fail_ts >= 0) {
					failed = true;
					failed_past = true;
				}
			}
		}
		tools.hidden.setVisible($("fan-health-dropdown"), failed);
		$("fan-health-led").className = (failed ? (failed_past ? "led-yellow" : "led-red") : "hidden");

		__fan_state = state;
		__renderAboutHardware();
	};

	var __renderAboutHardware = function() {
		let parts = [];
		if (__health_state !== null) {
			parts = [
				"Resources:" + __formatMisc(__health_state),
				"Temperature:" + __formatTemp(__health_state.temp),
				"Throttling:" + __formatThrottling(__health_state.throttling),
			];
		}
		if (__fan_state !== null) {
			parts.push("Fan:" + __formatFan(__fan_state));
		}
		$("about-hardware").innerHTML = parts.join("<hr>");
	};

	var __formatMisc = function(state) {
		return __formatUl([
			["CPU", tools.escape(`${state.cpu.percent}%`)],
			["MEM", tools.escape(`${state.mem.percent}%`)],
		]);
	};

	var __formatFan = function(state) {
		if (!state.monitored) {
			return __formatUl([["Status", "Not monitored"]]);
		} else if (state.state === null) {
			return __formatUl([["Status", __red("Not available")]]);
		} else {
			state = state.state;
			let kvs = [
				["Status",			(state.fan.ok ? __green("Ok") : __red("Failed"))],
				["Desired speed",	tools.escape(`${state.fan.speed}%`)],
				["PWM",				tools.escape(`${state.fan.pwm}`)],
			];
			if (state.hall.available) {
				kvs.push(["RPM", __colored(state.fan.ok, tools.escape(`${state.hall.rpm}`))]);
			}
			return __formatUl(kvs);
		}
	};

	var __formatTemp = function(temp) {
		let kvs = [];
		for (let field of Object.keys(temp).sort()) {
			kvs.push([
				tools.escape(field.toUpperCase()),
				tools.escape(`${temp[field]}`) + "&deg;C",
			]);
		}
		return __formatUl(kvs);
	};

	var __formatThrottling = function(throttling) {
		if (throttling !== null) {
			let kvs = [];
			for (let field of Object.keys(throttling.parsed_flags).sort()) {
				let flags = throttling.parsed_flags[field];
				let key = tools.upperFirst(field).replace("_", " ");
				let value = (flags["now"] ? __red("RIGHT NOW") : __green("No"));
				if (!throttling.ignore_past) {
					value += "; " + (flags["past"] ? __red("In the past") : __green("Never"));
				}
				kvs.push([tools.escape(key), value]);
			}
			return __formatUl(kvs);
		} else {
			return "NO DATA";
		}
	};

	var __setStateSystem = function(state) {
		let p = state.platform;
		let s = state.streamer;
		$("about-version").innerHTML = `
			Base: ${__commented(tools.escape(p.base))}
			<hr>
			Platform: ${__commented(tools.escape(p.model + "-" + p.video + "-" + p.board))}
			<hr>
			Serial: ${__commented(tools.escape(p.serial))}
			<hr>
			KVMD: ${__commented(tools.escape(state.kvmd.version))}
			<hr>
			Streamer: ${__commented(tools.escape(s.version + " (" + s.app + ")"))}
			${__formatStreamerFeatures(s.features)}
			<hr>
			${tools.escape(state.kernel.system)} kernel:
			${__formatUname(state.kernel)}
		`;
		$("kvmd-version-kvmd").innerText = state.kvmd.version;
		$("kvmd-version-streamer").innerText = s.version;
	};

	var __formatStreamerFeatures = function(features) {
		let kvs = [];
		for (let field of Object.keys(features).sort()) {
			kvs.push([
				tools.escape(field),
				(features[field] ? "Yes" : "No"),
			]);
		}
		return __formatUl(kvs);
	};

	var __formatUname = function(kernel) {
		let kvs = [];
		for (let field of Object.keys(kernel).sort()) {
			if (field !== "system") {
				kvs.push([
					tools.escape(tools.upperFirst(field)),
					tools.escape(kernel[field]),
				]);
			}
		}
		return __formatUl(kvs);
	};

	var __formatUl = function(kvs) {
		let html = "";
		for (let kv of kvs) {
			html += `<li>${kv[0]}: ${__commented(kv[1])}</li>`;
		}
		return `<ul>${html}</ul>`;
	};

	var __green = (html) => __colored(true, html);
	var __red = (html) => __colored(false, html);
	var __colored = (ok, html) => `<font color="${ok ? "green" : "red"}">${html}</font>`;
	var __commented = (html) => `<span class="code-comment">${html}</span>`;

	var __setStateExtras = function(state) {
		let show_hook = null;
		let close_hook = null;
		let has_webterm = (state.webterm && (state.webterm.enabled || state.webterm.started));
		if (has_webterm) {
			let loc = window.location;
			let base = `${loc.protocol}//${loc.host}${loc.pathname}${ROOT_PREFIX}`;
			// Tailing slash after state.webterm.path is added to avoid Nginx 301 redirect
			// when the location doesn't have tailing slash: "foo -> foo/".
			// Reverse proxy over PiKVM can be misconfigured to handle this.
			let url = base + state.webterm.path + "/?disableLeaveAlert=true";
			show_hook = function() {
				tools.info("Terminal opened: ", url);
				$("webterm-iframe").src = url;
			};
			close_hook = function() {
				tools.info("Terminal closed");
				$("webterm-iframe").src = "";
			};
		}
		tools.feature.setEnabled($("system-tool-webterm"), has_webterm);
		$("webterm-window").show_hook = show_hook;
		$("webterm-window").close_hook = close_hook;
	};

	__init__();
}
