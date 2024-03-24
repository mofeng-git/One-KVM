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


export function Recorder() {
	var self = this;

	/************************************************************************/

	var __ws = null;

	var __play_timer = null;
	var __recording = false;
	var __events = [];
	var __events_time = 0;
	var __last_event_ts = 0;

	var __init__ = function() {
		tools.el.setOnClick($("hid-recorder-record"), __startRecord);
		tools.el.setOnClick($("hid-recorder-stop"), __stopProcess);
		tools.el.setOnClick($("hid-recorder-play"), __playRecord);
		tools.el.setOnClick($("hid-recorder-clear"), __clearRecord);

		$("hid-recorder-new-script-file").onchange = __uploadScript;
		tools.el.setOnClick($("hid-recorder-upload"), () => $("hid-recorder-new-script-file").click());
		tools.el.setOnClick($("hid-recorder-download"), __downloadScript);
	};

	/************************************************************************/

	self.setSocket = function(ws) {
		if (ws !== __ws) {
			__ws = ws;
		}
		if (__ws === null) {
			__stopProcess();
		}
		__refresh();
	};

	self.recordWsEvent = function(event) {
		__recordEvent(event);
	};

	self.recordPrintEvent = function(text) {
		__recordEvent({"event_type": "print", "event": {"text": text}});
	};

	self.recordAtxButtonEvent = function(button) {
		__recordEvent({"event_type": "atx_button", "event": {"button": button}});
	};

	self.recordGpioSwitchEvent = function(channel, to) {
		__recordEvent({"event_type": "gpio_switch", "event": {"channel": channel, "state": to}});
	};

	self.recordGpioPulseEvent = function(channel) {
		__recordEvent({"event_type": "gpio_pulse", "event": {"channel": channel}});
	};

	var __recordEvent = function(event) {
		if (__recording) {
			let now = new Date().getTime();
			if (__last_event_ts) {
				let delay = now - __last_event_ts;
				__events.push({"event_type": "delay", "event": {"millis": delay}});
				__events_time += delay;
			}
			__last_event_ts = now;
			__events.push(event);
			__setCounters(__events.length, __events_time);
		}
	};

	var __startRecord = function() {
		__clearRecord();
		__recording = true;
		__refresh();
	};

	var __stopProcess = function() {
		if (__play_timer) {
			clearTimeout(__play_timer);
			__play_timer = null;
		}
		if (__recording) {
			__recording = false;
		}
		__refresh();
	};

	var __playRecord = function() {
		__play_timer = setTimeout(() => __runEvents(0), 0);
		__refresh();
	};

	var __clearRecord = function() {
		__events = [];
		__events_time = 0;
		__last_event_ts = 0;
		__refresh();
	};

	var __downloadScript = function() {
		let blob = new Blob([JSON.stringify(__events, undefined, 4)], {"type": "application/json"});
		let url = window.URL.createObjectURL(blob);
		let el_anchor = document.createElement("a");
		el_anchor.href = url;
		el_anchor.download = "script.json";
		el_anchor.click();
		window.URL.revokeObjectURL(url);
	};

	var __uploadScript = function() {
		let el_input = $("hid-recorder-new-script-file");
		let script_file = (el_input.files.length ? el_input.files[0] : null);
		if (script_file) {
			let reader = new FileReader();
			reader.onload = function () {
				let events = [];
				let events_time = 0;

				try {
					let raw_events = JSON.parse(reader.result);
					__checkType(raw_events, "object", "Base of script is not an objects list");

					for (let event of raw_events) {
						__checkType(event, "object", "Non-dict event");
						__checkType(event.event, "object", "Non-dict event");

						if (event.event_type === "delay") {
							__checkUnsigned(event.event.millis, "Non-unsigned delay");
							events_time += event.event.millis;

						} else if (event.event_type === "print") {
							__checkType(event.event.text, "string", "Non-string print text");

						} else if (event.event_type === "key") {
							__checkType(event.event.key, "string", "Non-string key code");
							__checkType(event.event.state, "boolean", "Non-bool key state");

						} else if (event.event_type === "mouse_button") {
							__checkType(event.event.button, "string", "Non-string mouse button code");
							__checkType(event.event.state, "boolean", "Non-bool mouse button state");

						} else if (event.event_type === "mouse_move") {
							__checkType(event.event.to, "object", "Non-object mouse move target");
							__checkInt(event.event.to.x, "Non-int mouse move X");
							__checkInt(event.event.to.y, "Non-int mouse move Y");

						} else if (event.event_type === "mouse_relative") {
							__checkMouseRelativeDelta(event.event.delta);
							__checkType(event.event.squash, "boolean", "Non-boolean squash");

						} else if (event.event_type === "mouse_wheel") {
							__checkType(event.event.delta, "object", "Non-object mouse wheel delta");
							__checkInt(event.event.delta.x, "Non-int mouse delta X");
							__checkInt(event.event.delta.y, "Non-int mouse delta Y");

						} else if (event.event_type === "atx_button") {
							__checkType(event.event.button, "string", "Non-string ATX button");

						} else if (event.event_type === "gpio_switch") {
							__checkType(event.event.channel, "string", "Non-string GPIO channel");
							__checkType(event.event.state, "boolean", "Non-bool GPIO state");

						} else if (event.event_type === "gpio_pulse") {
							__checkType(event.event.channel, "string", "Non-string GPIO channel");

						} else if (event.event_type === "delay_random") {
							__checkType(event.event.range, "object", "Non-object random delay range");
							__checkUnsigned(event.event.range.min, "Non-unsigned random delay range min");
							__checkUnsigned(event.event.range.max, "Non-unsigned random delay range max");
							__checkRangeMinMax(event.event.range, "Invalid random delay range");
							events_time += event.event.range.max;

						} else if (event.event_type === "mouse_move_random") { // Hack for pikvm/pikvm#1041
							__checkType(event.event.range, "object", "Non-object random mouse move range");
							__checkInt(event.event.range.min, "Non-int random mouse move range min");
							__checkInt(event.event.range.max, "Non-int random mouse move range max");
							__checkRangeMinMax(event.event.range, "Invalid random mouse move range");

						} else {
							throw `Unknown event type: ${event.event_type}`;
						}

						events.push(event);
					}

					__events = events;
					__events_time = events_time;
				} catch (err) {
					wm.error(`Invalid script: ${err}`);
				}

				el_input.value = "";
				__refresh();
			};
			reader.readAsText(script_file, "UTF-8");
		}
	};

	var __checkType = function(obj, type, msg) {
		if (typeof obj !== type) {
			throw msg;
		}
	};

	var __checkInt = function(obj, msg) {
		if (!Number.isInteger(obj)) {
			throw msg;
		}
	};

	var __checkUnsigned = function(obj, msg) {
		__checkInt(obj, msg);
		if (obj < 0) {
			throw msg;
		}
	};

	var __checkRangeMinMax = function(obj, msg) {
		if (obj.min > obj.max) {
			throw msg;
		}
	};

	var __checkArray = function (obj, msg) {
		if (!Array.isArray(obj)) {
			throw msg;
		}
	};

	var __checkMouseRelativeDelta = function(delta) {
		__checkArray(delta, "Non-array relative mouse delta");
		delta.forEach(element => {
			__checkType(element, "object", "Non-object relative mouse delta element");
			__checkInt(element.x, "Non-int mouse delta X");
			__checkInt(element.y, "Non-int mouse delta Y");
		});
	};

	var __runEvents = function(index, time=0) {
		while (index < __events.length) {
			__setCounters(__events.length - index + 1, __events_time - time);
			let event = __events[index];

			if (["delay", "delay_random"].includes(event.event_type)) {
				let millis = (
					event.event_type === "delay"
						? event.event.millis
						: tools.getRandomInt(event.event.range.min, event.event.range.max)
				);
				__play_timer = setTimeout(() => __runEvents(index + 1, time + millis), millis);
				return;

			} else if (event.event_type === "print") {
				tools.httpPost("/api/hid/print?limit=0", function(http) {
					if (http.status === 413) {
						wm.error("Too many text for paste!");
						__stopProcess();
					} else if (http.status !== 200) {
						wm.error("HID paste error:<br>", http.responseText);
						__stopProcess();
					} else if (http.status === 200) {
						__play_timer = setTimeout(() => __runEvents(index + 1, time), 0);
					}
				}, event.event.text, "text/plain");
				return;

			} else if (event.event_type === "atx_button") {
				tools.httpPost(`/api/atx/click?button=${event.event.button}`, function(http) {
					if (http.status !== 200) {
						wm.error("ATX error:<br>", http.responseText);
						__stopProcess();
					} else if (http.status === 200) {
						__play_timer = setTimeout(() => __runEvents(index + 1, time), 0);
					}
				});
				return;

			} else if (["gpio_switch", "gpio_pulse"].includes(event.event_type)) {
				let path = "/api/gpio";
				if (event.event_type === "gpio_switch") {
					path += `/switch?channel=${event.event.channel}&state=${event.event.to}`;
				} else { // gpio_pulse
					path += `/pulse?channel=${event.event.channel}`;
				}
				tools.httpPost(path, function(http) {
					if (http.status !== 200) {
						wm.error("GPIO error:<br>", http.responseText);
						__stopProcess();
					} else if (http.status === 200) {
						__play_timer = setTimeout(() => __runEvents(index + 1, time), 0);
					}
				});
				return;

			} else if (["key", "mouse_button", "mouse_move", "mouse_wheel", "mouse_relative"].includes(event.event_type)) {
				__ws.sendHidEvent(event);

			} else if (event.event_type === "mouse_move_random") {
				__ws.sendHidEvent({
					"event_type": "mouse_move",
					"event": {"to": {
						"x": tools.getRandomInt(event.event.range.min, event.event.range.max),
						"y": tools.getRandomInt(event.event.range.min, event.event.range.max),
					}},
				});
			}

			index += 1;
		}
		if ($("hid-recorder-loop-switch").checked) {
			setTimeout(() => __runEvents(0));
		} else {
			__stopProcess();
		}
	};

	var __refresh = function() {
		if (__play_timer) {
			$("hid-recorder-led").className = "led-yellow-rotating-fast";
			$("hid-recorder-led").title = "Playing...";
		} else if (__recording) {
			$("hid-recorder-led").className = "led-red-rotating-fast";
			$("hid-recorder-led").title = "Recording...";
		} else {
			$("hid-recorder-led").className = "led-gray";
			$("hid-recorder-led").title = "";
		}

		tools.el.setEnabled($("hid-recorder-record"), (__ws && !__play_timer && !__recording));
		tools.el.setEnabled($("hid-recorder-stop"), (__ws && (__play_timer || __recording)));
		tools.el.setEnabled($("hid-recorder-play"), (__ws && !__recording && __events.length));
		tools.el.setEnabled($("hid-recorder-clear"), (!__play_timer && !__recording && __events.length));
		tools.el.setEnabled($("hid-recorder-loop-switch"), (__ws && !__recording));

		tools.el.setEnabled($("hid-recorder-upload"), (!__play_timer && !__recording));
		tools.el.setEnabled($("hid-recorder-download"), (!__play_timer && !__recording && __events.length));

		__setCounters(__events.length, __events_time);
	};

	var __setCounters = function(events_count, events_time) {
		$("hid-recorder-time").innerHTML = tools.formatDuration(events_time);
		$("hid-recorder-events-count").innerHTML = events_count;
	};

	__init__();
}
