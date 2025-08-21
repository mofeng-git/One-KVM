/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
#    Copyright (C) 2023-2025  SilentWind <mofeng654321@hotmail.com>          #
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

import {Info} from "./info.js";
import {Recorder} from "./recorder.js";
import {Hid} from "./hid.js";
import {Paste} from "./paste.js";
import {Atx} from "./atx.js";
import {Msd} from "./msd.js";
import {Streamer} from "./stream.js";
import {Gpio} from "./gpio.js";
import {Ocr} from "./ocr.js";
import {Switch} from "./switch.js";


export function Session() {
	// var self = this;

	/************************************************************************/

	var __ws = null;

	var __ping_timer = null;
	var __missed_heartbeats = 0;

	var __info = new Info();
	var __streamer = new Streamer();
	var __recorder = new Recorder();
	var __hid = new Hid(__streamer.getGeometry, __recorder);
	var __paste = new Paste(__recorder);
	var __atx = new Atx(__recorder);
	var __msd = new Msd();
	var __gpio = new Gpio(__recorder);
	var __ocr = new Ocr(__streamer.getGeometry);
	var __switch = new Switch();

	var __init__ = function() {
		__streamer.ensureDeps(() => __startSession());
	};

	/************************************************************************/

	var __startSession = function() {
		$("link-led").className = "led-yellow";
		$("link-led").title = "Connecting...";

		tools.httpGet("api/auth/check", null, function(http) {
			if (http.status === 200) {
				__ws = new WebSocket(tools.makeWsUrl("api/ws"));
				__ws.sendHidEvent = (ev) => __sendHidEvent(__ws, ev.event_type, ev.event);
				__ws.binaryType = "arraybuffer";
				__ws.onopen = __wsOpenHandler;
				__ws.onmessage = async (ev) => {
					if (typeof ev.data === "string") {
						ev = JSON.parse(ev.data);
						__wsJsonHandler(ev.event_type, ev.event);
					} else { // Binary
						__wsBinHandler(ev.data);
					}
				};
				__ws.onerror = __wsErrorHandler;
				__ws.onclose = __wsCloseHandler;
			} else if (http.status === 401 || http.status === 403) {
				window.onbeforeunload = () => null;
				wm.error("Unexpected logout occured, please login again").then(function() {
					tools.currentOpen("login");
				});
			} else {
				__wsCloseHandler(null);
			}
		});
	};

	var __wsOpenHandler = function(ev) {
		tools.debug("Session: socket opened:", ev);
		$("link-led").className = "led-green";
		$("link-led").title = "Connected";
		__recorder.setSocket(__ws);
		__hid.setSocket(__ws);
		__missed_heartbeats = 0;
		__ping_timer = setInterval(__pingServer, 1000);
	};

	var __wsBinHandler = function(data) {
		data = new Uint8Array(data);
		if (data[0] === 255) { // Pong
			__missed_heartbeats = 0;
		}
	};

	var __wsJsonHandler = function(ev_type, ev) {
		switch (ev_type) {
			case "info": __info.setState(ev); break;
			case "gpio": __gpio.setState(ev); break;
			case "hid": __hid.setState(ev); break;
			case "hid_keymaps": __paste.setState(ev); break;
			case "atx": __atx.setState(ev); break;
			case "streamer": __streamer.setState(ev); break;
			case "ocr": __ocr.setState(ev); break;

			case "msd":
				if (ev.online === false) {
					__switch.setMsdConnected(false);
				} else if (ev.drive !== undefined) {
					__switch.setMsdConnected(ev.drive.connected);
				}
				__msd.setState(ev);
				break;

			case "switch":
				if (ev.model) {
					__atx.setHasSwitch(ev.model.ports.length > 0);
				}
				__switch.setState(ev);
				break;
		}
	};

	var __wsErrorHandler = function(ev) {
		tools.error("Session: socket error:", ev);
		if (__ws) {
			__ws.onclose = null;
			__ws.close();
			__wsCloseHandler(null);
		}
	};

	var __wsCloseHandler = function(ev) {
		tools.debug("Session: socket closed:", ev);
		$("link-led").className = "led-gray";

		if (__ping_timer) {
			clearInterval(__ping_timer);
			__ping_timer = null;
		}

		__gpio.setState(null);
		__hid.setSocket(null); // auto setState(null);
		__paste.setState(null);
		__atx.setState(null);
		__msd.setState(null);
		__streamer.setState(null);
		__ocr.setState(null);
		__recorder.setSocket(null);
		__switch.setState(null);
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
			__ws.send(new Uint8Array([0]));
		} catch (ex) {
			__wsErrorHandler(ex.message);
		}
	};

	var __ascii_encoder = new TextEncoder("ascii");

	var __sendHidEvent = function(ws, ev_type, ev) {
		if (ev_type === "key") {
			let data = __ascii_encoder.encode("\x01\x00" + ev.key);
			data[1] = (ev.state ? 1 : 0);
			if (ev.finish === true) { // Optional
				data[1] |= 0x02;
			}
			ws.send(data);

		} else if (ev_type === "mouse_button") {
			let data = __ascii_encoder.encode("\x02\x00" + ev.button);
			data[1] = (ev.state ? 1 : 0);
			ws.send(data);

		} else if (ev_type === "mouse_move") {
			let data = new Uint8Array([
				3,
				(ev.to.x >> 8) & 0xFF, ev.to.x & 0xFF,
				(ev.to.y >> 8) & 0xFF, ev.to.y & 0xFF,
			]);
			ws.send(data);

		} else if (ev_type === "mouse_relative" || ev_type === "mouse_wheel") {
			let data;
			if (Array.isArray(ev.delta)) {
				data = new Int8Array(2 + ev.delta.length * 2);
				let index = 0;
				for (let delta of ev.delta) {
					data[index + 2] = delta["x"];
					data[index + 3] = delta["y"];
					index += 2;
				}
			} else {
				data = new Int8Array([0, 0, ev.delta.x, ev.delta.y]);
			}
			data[0] = (ev_type === "mouse_relative" ? 4 : 5);
			data[1] = (ev.squash ? 1 : 0);
			ws.send(data);
		}
	};

	__init__();
}
