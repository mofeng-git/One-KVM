/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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

import {Recorder} from "./recorder.js";
import {Hid} from "./hid.js";
import {Atx} from "./atx.js";
import {Msd} from "./msd.js";
import {Streamer} from "./stream.js";
import {Gpio} from "./gpio.js";
import {Ocr} from "./ocr.js";


export function Session() {
	// var self = this;

	/************************************************************************/

	var __ws = null;

	var __ping_timer = null;
	var __missed_heartbeats = 0;

	var __streamer = new Streamer();
	var __recorder = new Recorder();
	var __hid = new Hid(__streamer.getGeometry, __recorder);
	var __atx = new Atx(__recorder);
	var __msd = new Msd();
	var __gpio = new Gpio(__recorder);
	var __ocr = new Ocr(__streamer.getGeometry);

	var __info_hw_state = null;
	var __info_fan_state = null;

	var __init__ = function() {
		__startSession();
	};

	/************************************************************************/

	var __setAboutInfoMeta = function(state) {
		if (state !== null) {
			let text = JSON.stringify(state, undefined, 4).replace(/ /g, "&nbsp;").replace(/\n/g, "<br>");
			$("about-meta").innerHTML = `
				<span class="code-comment">// The PiKVM metadata.<br>
				// You can get this JSON using handle <a target="_blank" href="/api/info?fields=meta">/api/info?fields=meta</a>.<br>
				// In the standard configuration this data<br>
				// is specified in the file /etc/kvmd/meta.yaml.</span><br>
				<br>
				${text}
			`;
			if (state.server && state.server.host) {
				$("kvmd-meta-server-host").innerHTML = `Server: ${state.server.host}`;
				document.title = `PiKVM Session: ${state.server.host}`;
			} else {
				$("kvmd-meta-server-host").innerHTML = "";
				document.title = "PiKVM Session";
			}

			// Don't use this option, it may be removed in any time
			if (state.web && state.web.confirm_session_exit === false) {
				window.onbeforeunload = null; // See main.js
			}
		}
	};

	var __setAboutInfoHw = function(state) {
		if (state.health.throttling !== null) {
			let flags = state.health.throttling.parsed_flags;
			let undervoltage = (flags.undervoltage.now || flags.undervoltage.past);
			let freq_capped = (flags.freq_capped.now || flags.freq_capped.past);

			tools.hidden.setVisible($("hw-health-dropdown"), (undervoltage || freq_capped));
			$("hw-health-undervoltage-led").className = (undervoltage ? (flags.undervoltage.now ? "led-red" : "led-yellow") : "hidden");
			$("hw-health-overheating-led").className = (freq_capped ? (flags.freq_capped.now ? "led-red" : "led-yellow") : "hidden");
			tools.hidden.setVisible($("hw-health-message-undervoltage"), undervoltage);
			tools.hidden.setVisible($("hw-health-message-overheating"), freq_capped);
		}
		__info_hw_state = state;
		__renderAboutInfoHardware();
	};

	var __setAboutInfoFan = function(state) {
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

		__info_fan_state = state;
		__renderAboutInfoHardware();
	};

	var __renderAboutInfoHardware = function() {
		let html = "";
		if (__info_hw_state !== null) {
			html += `
				Platform:
				${__formatPlatform(__info_hw_state.platform)}
				<hr>
				Temperature:
				${__formatTemp(__info_hw_state.health.temp)}
				<hr>
				Throttling:
				${__formatThrottling(__info_hw_state.health.throttling)}
			`;
		}
		if (__info_fan_state !== null) {
			if (html.length > 0) {
				html += "<hr>";
			}
			html += `
				Fan:
				${__formatFan(__info_fan_state)}
			`;
		}
		$("about-hardware").innerHTML = html;
	};

	var __formatPlatform = function(state) {
		return __formatUl([["Base", state.base], ["Serial", state.serial]]);
	};

	var __formatFan = function(state) {
		if (!state.monitored) {
			return __formatUl([["Status", "Not monitored"]]);
		} else if (state.state === null) {
			return __formatUl([["Status", __colored("red", "Not available")]]);
		} else {
			state = state.state;
			let pairs = [
				["Status", (state.fan.ok ? __colored("green", "Ok") : __colored("red", "Failed"))],
				["Desired speed", `${state.fan.speed}%`],
				["PWM", `${state.fan.pwm}`],
			];
			if (state.hall.available) {
				pairs.push(["RPM", __colored((state.fan.ok ? "green" : "red"), state.hall.rpm)]);
			}
			return __formatUl(pairs);
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
				let flags = throttling.parsed_flags[field];
				pairs.push([
					tools.upperFirst(field).replace("_", " "),
					(flags["now"] ? __colored("red", "RIGHT NOW") : __colored("green", "No"))
					+ "; " +
					(flags["past"] ? __colored("red", "In the past") : __colored("green", "Never")),
				]);
			}
			return __formatUl(pairs);
		} else {
			return "NO DATA";
		}
	};

	var __colored = function(color, text) {
		return `<font color="${color}">${text}</font>`;
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

	var __setExtras = function(state) {
		let show_hook = null;
		let close_hook = null;
		let has_webterm = (state.webterm && (state.webterm.enabled || state.webterm.started));
		if (has_webterm) {
			let path = "/" + state.webterm.path;
			show_hook = function() {
				tools.info("Terminal opened: ", path);
				$("webterm-iframe").src = path;
			};
			close_hook = function() {
				tools.info("Terminal closed");
				$("webterm-iframe").src = "";
			};
		}
		tools.feature.setEnabled($("system-tool-webterm"), has_webterm);
		$("webterm-window").show_hook = show_hook;
		$("webterm-window").close_hook = close_hook;

		__streamer.setJanusEnabled(
			(state.janus && (state.janus.enabled || state.janus.started))
			|| (state.janus_static && (state.janus_static.enabled || state.janus_static.started))
		);
	};

	var __startSession = function() {
		$("link-led").className = "led-yellow";
		$("link-led").title = "Connecting...";

		let http = tools.makeRequest("GET", "/api/auth/check", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					__ws = new WebSocket(`${tools.is_https ? "wss" : "ws"}://${location.host}/api/ws`);
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
		__recorder.setSocket(__ws);
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
			case "info_fan_state": __setAboutInfoFan(data.event); break;
			case "info_system_state": __setAboutInfoSystem(data.event); break;
			case "info_extras_state": __setExtras(data.event); break;
			case "gpio_model_state": __gpio.setModel(data.event); break;
			case "gpio_state": __gpio.setState(data.event); break;
			case "hid_keymaps_state": __hid.setKeymaps(data.event); break;
			case "hid_state": __hid.setState(data.event); break;
			case "atx_state": __atx.setState(data.event); break;
			case "msd_state": __msd.setState(data.event); break;
			case "streamer_state": __streamer.setState(data.event); break;
			case "streamer_ocr_state": __ocr.setState(data.event); break;
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

		__ocr.setState(null);
		__gpio.setState(null);
		__hid.setSocket(null);
		__recorder.setSocket(null);
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
