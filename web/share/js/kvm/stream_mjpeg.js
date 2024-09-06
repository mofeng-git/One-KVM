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


export function MjpegStreamer(__setActive, __setInactive, __setInfo) {
	var self = this;

	/************************************************************************/

	var __key = tools.makeId();
	var __id = "";
	var __fps = -1;
	var __state = null;

	var __timer = null;
	var __timer_retries = 0;

	/************************************************************************/

	self.getName = () => "MJPEG";
	self.getMode = () => "mjpeg";

	self.getResolution = function() {
		let el = $("stream-image");
		return {
			"real_width": el.naturalWidth,
			"real_height": el.naturalHeight,
			"view_width": el.offsetWidth,
			"view_height": el.offsetHeight,
		};
	};

	self.ensureStream = function(state) {
		if (state) {
			__state = state;
			__findId();
			if (__id.length > 0 && __id in __state.stream.clients_stat) {
				__setStreamActive();
				__stopChecking();
			} else {
				__ensureChecking();
			}
		} else {
			__stopChecking();
			__setStreamInactive();
		}
	};

	self.stopStream = function() {
		self.ensureStream(null);
		let blank = "/share/png/blank-stream.png";
		if (!String.prototype.endsWith.call($("stream-image").src, blank)) {
			$("stream-image").src = blank;
		}
	};

	var __setStreamActive = function() {
		let old_fps = __fps;
		__fps = __state.stream.clients_stat[__id].fps;
		if (old_fps < 0) {
			__logInfo("Active");
			__setActive();
		}
		__setInfo(true, __state.source.online, `${__fps} fps dynamic`);
	};

	var __setStreamInactive = function() {
		let old_fps = __fps;
		__key = tools.makeId();
		__id = "";
		__fps = -1;
		__state = null;
		if (old_fps >= 0) {
			__logInfo("Inactive");
			__setInactive();
			__setInfo(false, false, "");
		}
	};

	var __ensureChecking = function() {
		if (!__timer) {
			__timer_retries = 10;
			__timer = setInterval(__checkStream, 100);
		}
	};

	var __stopChecking = function() {
		if (__timer) {
			clearInterval(__timer);
		}
		__timer = null;
		__timer_retries = 0;
	};

	var __findId = function() {
		let stream_client = tools.cookies.get("stream_client");
		if (__id.length === 0 && stream_client && stream_client.startsWith(__key + "/")) {
			__logInfo("Found acceptable stream_client cookie:", stream_client);
			__id = stream_client.slice(stream_client.indexOf("/") + 1);
		}
	};

	var __checkStream = function() {
		__findId();

		if (__id.legnth > 0 && __id in __state.stream.clients_stat) {
			__setStreamActive();
			__stopChecking();

		} else if (__id.length > 0 && __timer_retries >= 0) {
			__timer_retries -= 1;

		} else {
			__setStreamInactive();
			__stopChecking();

			let path = `/streamer/stream?key=${__key}`;
			if (tools.browser.is_safari || tools.browser.is_ios) {
				// uStreamer fix for WebKit
				__logInfo("Using dual_final_frames=1 to fix WebKit bugs");
				path += "&dual_final_frames=1";
			} else if (tools.browser.is_chrome || tools.browser.is_blink) {
				// uStreamer fix for Blink https://bugs.chromium.org/p/chromium/issues/detail?id=527446
				__logInfo("Using advance_headers=1 to fix Blink bugs");
				path += "&advance_headers=1";
			}

			__logInfo("Refreshing ...");
			$("stream-image").src = path;
		}
	};

	var __logInfo = (...args) => tools.info("Stream [MJPEG]:", ...args);
}
