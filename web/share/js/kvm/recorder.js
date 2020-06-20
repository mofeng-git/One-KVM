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


import {tools, $} from "../tools.js";
import {wm} from "../wm.js";


export function Recorder() {
	var self = this;

	/************************************************************************/

	var __ws = null;

	var __play_timer = null;
	var __recording = false;
	var __record = [];
	var __record_time = 0;
	var __last_event_ts = 0;

	var __init__ = function() {
		tools.setOnClick($("hid-recorder-record"), __startRecord);
		tools.setOnClick($("hid-recorder-stop"), __stopProcess);
		tools.setOnClick($("hid-recorder-play"), __playRecord);
		tools.setOnClick($("hid-recorder-clear"), __clearRecord);

		$("hid-recorder-new-script-file").onchange = __uploadScript;
		tools.setOnClick($("hid-recorder-upload"), () => $("hid-recorder-new-script-file").click());
		tools.setOnClick($("hid-recorder-download"), __downloadScript);
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

	var __recordEvent = function(event) {
		if (__recording) {
			let now = new Date().getTime();
			if (__last_event_ts) {
				let delay = now - __last_event_ts;
				__record.push({"event_type": "delay", "event": {"millis": delay}});
				__record_time += delay;
			}
			__last_event_ts = now;
			__record.push(event);
			__setCounters(__record.length, __record_time);
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
		__record = [];
		__record_time = 0;
		__last_event_ts = 0;
		__refresh();
	};

	var __downloadScript = function() {
		let blob = new Blob([JSON.stringify(__record, undefined, 4)], {"type": "application/json"});
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
				let record = [];
				let record_time = 0;

				try {
					let raw_record = JSON.parse(reader.result);
					console.log(typeof raw_record);
					console.log(raw_record);
					__checkType(raw_record, "object", "Base of script is not an objects list");

					for (let event of raw_record) {
						__checkType(event, "object", "Non-dict event");
						__checkType(event.event, "object", "Non-dict event");

						if (event.event_type === "delay") {
							__checkInt(event.event.millis, "Non-integer delay");
							if (event.event.millis < 0) {
								throw "Negative delay";
							}
							record_time += event.event.millis;
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
						} else if (event.event_type === "mouse_wheel") {
							__checkType(event.event.delta, "object", "Non-object mouse wheel delta");
							__checkInt(event.event.delta.x, "Non-int mouse delta X");
							__checkInt(event.event.delta.y, "Non-int mouse delta Y");
						} else {
							throw "Unknown event type";
						}

						record.push(event);
					}

					__record = record;
					__record_time = record_time;
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

	var __runEvents = function(index, time=0) {
		while (index < __record.length) {
			__setCounters(__record.length - index + 1, __record_time - time);
			let event = __record[index];
			if (event.event_type === "delay") {
				__play_timer = setTimeout(() => __runEvents(index + 1, time + event.event.millis), event.event.millis);
				return;
			} else if (event.event_type === "print") {
				let http = tools.makeRequest("POST", "/api/hid/print?limit=0", function() {
					if (http.readyState === 4) {
						if (http.status === 413) {
							wm.error("Too many text for paste!");
							__stopProcess();
						} else if (http.status !== 200) {
							wm.error("HID paste error:<br>", http.responseText);
							__stopProcess();
						} else if (http.status === 200) {
							__play_timer = setTimeout(() => __runEvents(index + 1, time), 0);
						}
					}
				}, event.event.text, "text/plain");
				return;
			} else if (["key", "mouse_button", "mouse_move", "mouse_wheel"].includes(event.event_type)) {
				__ws.send(JSON.stringify(event));
			}
			index += 1;
		}
		__stopProcess();
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

		wm.switchEnabled($("hid-recorder-record"), (__ws && !__play_timer && !__recording));
		wm.switchEnabled($("hid-recorder-stop"), (__ws && (__play_timer || __recording)));
		wm.switchEnabled($("hid-recorder-play"), (__ws && !__recording && __record.length));
		wm.switchEnabled($("hid-recorder-clear"), (!__play_timer && !__recording && __record.length));
		wm.switchEnabled($("hid-recorder-upload"), (!__play_timer && !__recording));
		wm.switchEnabled($("hid-recorder-download"), (!__play_timer && !__recording && __record.length));

		__setCounters(__record.length, __record_time);
	};

	var __setCounters = function(events_count, time) {
		$("hid-recorder-time").innerHTML = tools.formatDuration(time);
		$("hid-recorder-events-count").innerHTML = events_count;
	};

	__init__();
}
