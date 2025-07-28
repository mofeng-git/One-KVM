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

	self.recordWsEvent = function(ev) {
		__recordEvent(ev);
	};

	self.recordPrintEvent = function(text, keymap, delay) {
		__recordEvent({"event_type": "print", "event": {"text": text, "keymap": keymap, "delay": delay}});
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

	var __recordEvent = function(ev) {
		if (__recording) {
			let now = new Date().getTime();
			if (__last_event_ts) {
				let delay = now - __last_event_ts;
				__events.push({"event_type": "delay", "event": {"millis": delay}});
				__events_time += delay;
			}
			__last_event_ts = now;
			__events.push(ev);
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

					for (let ev of raw_events) {
						__checkType(ev, "object", "Non-dict event");
						__checkType(ev.event, "object", "Non-dict event");

						if (ev.event_type === "delay") {
							__checkUnsigned(ev.event.millis, "Non-unsigned delay");
							events_time += ev.event.millis;

						} else if (ev.event_type === "print") {
							__checkType(ev.event.text, "string", "Non-string print text");
							if (ev.event.keymap !== undefined) {
								__checkType(ev.event.keymap, "string", "Non-string keymap");
							}
							if (ev.event.slow !== undefined) {
								__checkType(ev.event.slow, "boolean", "Non-bool slow");
							}
							if (ev.event.delay !== undefined) {
								__checkInt(ev.event.delay, "Non-int delay");
							}

						} else if (ev.event_type === "key") {
							__checkType(ev.event.key, "string", "Non-string key code");
							__checkType(ev.event.state, "boolean", "Non-bool key state");

						} else if (ev.event_type === "mouse_button") {
							__checkType(ev.event.button, "string", "Non-string mouse button code");
							__checkType(ev.event.state, "boolean", "Non-bool mouse button state");

						} else if (ev.event_type === "mouse_move") {
							__checkType(ev.event.to, "object", "Non-object mouse move target");
							__checkInt(ev.event.to.x, "Non-int mouse move X");
							__checkInt(ev.event.to.y, "Non-int mouse move Y");

						} else if (ev.event_type === "mouse_relative") {
							__checkMouseRelativeDelta(ev.event.delta);
							__checkType(ev.event.squash, "boolean", "Non-boolean squash");

						} else if (ev.event_type === "mouse_wheel") {
							__checkType(ev.event.delta, "object", "Non-object mouse wheel delta");
							__checkInt(ev.event.delta.x, "Non-int mouse delta X");
							__checkInt(ev.event.delta.y, "Non-int mouse delta Y");

						} else if (ev.event_type === "atx_button") {
							__checkType(ev.event.button, "string", "Non-string ATX button");

						} else if (ev.event_type === "gpio_switch") {
							__checkType(ev.event.channel, "string", "Non-string GPIO channel");
							__checkType(ev.event.state, "boolean", "Non-bool GPIO state");

						} else if (ev.event_type === "gpio_pulse") {
							__checkType(ev.event.channel, "string", "Non-string GPIO channel");

						} else if (ev.event_type === "delay_random") {
							__checkType(ev.event.range, "object", "Non-object random delay range");
							__checkUnsigned(ev.event.range.min, "Non-unsigned random delay range min");
							__checkUnsigned(ev.event.range.max, "Non-unsigned random delay range max");
							__checkRangeMinMax(ev.event.range, "Invalid random delay range");
							events_time += ev.event.range.max;

						} else if (ev.event_type === "mouse_move_random") { // Hack for pikvm/pikvm#1041
							__checkType(ev.event.range, "object", "Non-object random mouse move range");
							__checkInt(ev.event.range.min, "Non-int random mouse move range min");
							__checkInt(ev.event.range.max, "Non-int random mouse move range max");
							__checkRangeMinMax(ev.event.range, "Invalid random mouse move range");

						} else {
							throw `Unknown event type: ${ev.event_type}`;
						}

						events.push(ev);
					}

					__events = events;
					__events_time = events_time;
				} catch (ex) {
					wm.error("Invalid script", `${ex}`);
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
			let ev = __events[index];

			if (["delay", "delay_random"].includes(ev.event_type)) {
				let millis = (
					ev.event_type === "delay"
						? ev.event.millis
						: tools.getRandomInt(ev.event.range.min, ev.event.range.max)
				);
				__play_timer = setTimeout(() => __runEvents(index + 1, time + millis), millis);
				return;

			} else if (ev.event_type === "print") {
				let params = {"limit": 0};
				if (ev.event.keymap !== undefined) {
					params["keymap"] = ev.event.keymap;
				}
				if (ev.event.slow !== undefined) {
					params["slow"] = ev.event.slow;
				}
				if (ev.event.delay !== undefined) {
					params["delay"] = ev.event.delay / 1000;
				}
				tools.httpPost("api/hid/print", params, function(http) {
					if (http.status === 413) {
						wm.error("Too many text for paste!");
						__stopProcess();
					} else if (http.status !== 200) {
						wm.error("HID paste error", http.responseText);
						__stopProcess();
					} else if (http.status === 200) {
						__play_timer = setTimeout(() => __runEvents(index + 1, time), 0);
					}
				}, ev.event.text, "text/plain");
				return;

			} else if (ev.event_type === "atx_button") {
				tools.httpPost("api/atx/click", {"button": ev.event.button}, function(http) {
					if (http.status !== 200) {
						wm.error("ATX error", http.responseText);
						__stopProcess();
					} else if (http.status === 200) {
						__play_timer = setTimeout(() => __runEvents(index + 1, time), 0);
					}
				});
				return;

			} else if (["gpio_switch", "gpio_pulse"].includes(ev.event_type)) {
				let path = "api/gpio";
				let params = {"channel": ev.event.channel};
				if (ev.event_type === "gpio_switch") {
					path += "/switch";
					params["state"] = ev.event.to;
				} else { // gpio_pulse
					path += "/pulse";
				}
				tools.httpPost(path, params, function(http) {
					if (http.status !== 200) {
						wm.error("GPIO error", http.responseText);
						__stopProcess();
					} else if (http.status === 200) {
						__play_timer = setTimeout(() => __runEvents(index + 1, time), 0);
					}
				});
				return;

			} else if (ev.event_type === "key") {
				ev.event.finish = $("hid-keyboard-bad-link-switch").checked;
				__ws.sendHidEvent(ev);

			} else if (["mouse_button", "mouse_move", "mouse_wheel", "mouse_relative"].includes(ev.event_type)) {
				__ws.sendHidEvent(ev);

			} else if (ev.event_type === "mouse_move_random") {
				__ws.sendHidEvent({
					"event_type": "mouse_move",
					"event": {"to": {
						"x": tools.getRandomInt(ev.event.range.min, ev.event.range.max),
						"y": tools.getRandomInt(ev.event.range.min, ev.event.range.max),
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
