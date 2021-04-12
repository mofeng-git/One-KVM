/*****************************************************************************
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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

import {Hid} from "./hid.js";
import {Atx} from "./atx.js";
import {Msd} from "./msd.js";
import {Streamer} from "./stream.js";
import {WakeOnLan} from "./wol.js";
import {Gpio} from "./gpio.js";


export function Session() {
	// var self = this;

	/************************************************************************/

	var __ws = null;

	var __ping_timer = null;
	var __missed_heartbeats = 0;

	var __hid = new Hid();
	var __atx = new Atx();
	var __msd = new Msd();
	var __streamer = new Streamer(__hid);
	var __wol = new WakeOnLan();
	var __gpio = new Gpio();

	var __init__ = function() {
		__startSession();
	};

	/************************************************************************/

	var __setAboutInfoMeta = function(state) {
		if (state !== null) {
			let text = JSON.stringify(state, undefined, 4).replace(/ /g, "&nbsp;").replace(/\n/g, "<br>");
			$("about-meta").innerHTML = `
				<span class="code-comment">// The Pi-KVM metadata.<br>
				// You can get this JSON using handle <a target="_blank" href="/api/info?fields=meta">/api/info?fields=meta</a>.<br>
				// In the standard configuration this data<br>
				// is specified in the file /etc/kvmd/meta.yaml.</span><br>
				<br>
				${text}
			`;
			if (state.server && state.server.host) {
				$("kvmd-meta-server-host").innerHTML = `Server: ${state.server.host}`;
				document.title = `Pi-KVM Session: ${state.server.host}`;
			} else {
				$("kvmd-meta-server-host").innerHTML = "";
				document.title = "Pi-KVM Session";
			}

			// Don't use this option, it may be removed in any time
			if (state.web && state.web.confirm_session_exit === false) {
				window.onbeforeunload = null; // See main.js
			}
		}
	};

	var __setAboutInfoHw = function(state) {
		$("about-hw").innerHTML = `
			Platform base: <span class="code-comment">${state.platform.base}</span><br>
			<hr>
			Temperature:
			${__formatTemp(state.health.temp)}
			<hr>
			Throttling:
			${__formatThrottling(state.health.throttling)}
		`;

		if (state.health.throttling !== null) {
			let flags = state.health.throttling.parsed_flags;
			let undervoltage = (flags.undervoltage.now || flags.undervoltage.past);
			let freq_capped = (flags.freq_capped.now || flags.freq_capped.past);

			tools.hiddenSetVisible($("hw-health-dropdown"), (undervoltage || freq_capped));
			$("hw-health-undervoltage-led").className = (undervoltage ? (flags.undervoltage.now ? "led-red" : "led-yellow") : "hidden");
			$("hw-health-overheating-led").className = (freq_capped ? (flags.freq_capped.now ? "led-red" : "led-yellow") : "hidden");
			tools.hiddenSetVisible($("hw-health-message-undervoltage"), undervoltage);
			tools.hiddenSetVisible($("hw-health-message-overheating"), freq_capped);
		}
	};

	var __formatTemp = function(temp) {
		let pairs = [];
		for (let field of Object.keys(temp).sort()) {
			pairs.push([field.toUpperCase(), `${temp[field]}&deg;C`]);
		}
		return __formatUl(pairs);
	};

	var __formatThrottling = function(throttling) {
		if (throttling !== null) {
			let pairs = [];
			for (let field of Object.keys(throttling.parsed_flags).sort()) {
				pairs.push([
					tools.upperFirst(field).replace("_", " "),
					__formatThrottleError(throttling.parsed_flags[field]),
				]);
			}
			return __formatUl(pairs);
		} else {
			return "NO DATA";
		}
	};

	var __formatThrottleError = function(flags) {
		let colored = ((color, text) => `<font color="${color}">${text}</font>`);
		return `
			${flags["now"] ? colored("red", "RIGHT NOW") : colored("green", "No")};
			${flags["past"] ? colored("red", "In the past") : colored("green", "Never")}
		`;
	};

	var __setAboutInfoSystem = function(state) {
		$("about-version").innerHTML = `
			KVMD: <span class="code-comment">${state.kvmd.version}</span><br>
			<hr>
			Streamer: <span class="code-comment">${state.streamer.version} (${state.streamer.app})</span>
			${__formatStreamerFeatures(state.streamer.features)}
			<hr>
			${state.kernel.system} kernel:
			${__formatUname(state.kernel)}
		`;
	};

	var __formatStreamerFeatures = function(features) {
		let pairs = [];
		for (let field of Object.keys(features).sort()) {
			pairs.push([field, (features[field] ? "Yes" : "No")]);
		}
		return __formatUl(pairs);
	};

	var __formatUname = function(kernel) {
		let pairs = [];
		for (let field of Object.keys(kernel).sort()) {
			if (field !== "system") {
				pairs.push([tools.upperFirst(field), kernel[field]]);
			}
		}
		return __formatUl(pairs);
	};

	var __formatUl = function(pairs) {
		let text = "<ul>";
		for (let pair of pairs) {
			text += `<li>${pair[0]}: <span class="code-comment">${pair[1]}</span></li>`;
		}
		return text + "</ul>";
	};

	var __startSession = function() {
		$("link-led").className = "led-yellow";
		$("link-led").title = "Connecting...";

		let http = tools.makeRequest("GET", "/api/auth/check", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					let proto = (location.protocol === "https:" ? "wss" : "ws");
					__ws = new WebSocket(`${proto}://${location.host}/api/ws`);
					__ws.onopen = __wsOpenHandler;
					__ws.onmessage = __wsMessageHandler;
					__ws.onerror = __wsErrorHandler;
					__ws.onclose = __wsCloseHandler;
				} else if (http.status === 401 || http.status === 403) {
					window.onbeforeunload = () => null;
					wm.error("Unexpected logout occured, please login again").then(function() {
						document.location.href = "/login";
					});
				} else {
					__wsCloseHandler(null);
				}
			}
		});
	};

	var __wsOpenHandler = function(event) {
		tools.debug("Session: socket opened:", event);
		$("link-led").className = "led-green";
		$("link-led").title = "Connected";
		__hid.setSocket(__ws);
		__missed_heartbeats = 0;
		__ping_timer = setInterval(__pingServer, 1000);
	};

	var __wsMessageHandler = function(event) {
		// tools.debug("Session: received socket data:", event.data);
		let data = JSON.parse(event.data);
		switch (data.event_type) {
			case "pong": __missed_heartbeats = 0; break;
			case "info_meta_state": __setAboutInfoMeta(data.event); break;
			case "info_hw_state": __setAboutInfoHw(data.event); break;
			case "info_system_state": __setAboutInfoSystem(data.event); break;
			case "wol_state": __wol.setState(data.event); break;
			case "gpio_model_state": __gpio.setModel(data.event); break;
			case "gpio_state": __gpio.setState(data.event); break;
			case "hid_state": __hid.setState(data.event); break;
			case "atx_state": __atx.setState(data.event); break;
			case "msd_state": __msd.setState(data.event); break;
			case "streamer_state": __streamer.setState(data.event); break;
		}
	};

	var __wsErrorHandler = function(event) {
		tools.error("Session: socket error:", event);
		if (__ws) {
			__ws.onclose = null;
			__ws.close();
			__wsCloseHandler(null);
		}
	};

	var __wsCloseHandler = function(event) {
		tools.debug("Session: socket closed:", event);

		$("link-led").className = "led-gray";

		if (__ping_timer) {
			clearInterval(__ping_timer);
			__ping_timer = null;
		}

		__gpio.setState(null);
		__hid.setSocket(null);
		__atx.setState(null);
		__msd.setState(null);
		__streamer.setState(null);
		__ws = null;

		setTimeout(function() {
			$("link-led").className = "led-yellow";
			setTimeout(__startSession, 500);
		}, 500);
	};

	var __pingServer = function() {
		try {
			__missed_heartbeats += 1;
			if (__missed_heartbeats >= 15) {
				throw new Error("Too many missed heartbeats");
			}
			__ws.send(JSON.stringify({"event_type": "ping", "event": {}}));
		} catch (err) {
			tools.error("Session: ping error:", err.message);
			if (__ws) {
				__ws.onclose = null;
				__ws.close();
				__wsCloseHandler(null);
			}
		}
	};

	__init__();
}
