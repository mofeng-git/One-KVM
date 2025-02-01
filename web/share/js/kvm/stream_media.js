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


export function MediaStreamer(__setActive, __setInactive, __setInfo) {
	var self = this;

	/************************************************************************/

	var __stop = false;
	var __ensuring = false;

	var __ws = null;
	var __ping_timer = null;
	var __missed_heartbeats = 0;
	var __decoder = null;
	var __codec = "";
	var __canvas = $("stream-canvas");
	var __ctx = __canvas.getContext("2d");

	var __state = null;
	var __frames = 0;

	/************************************************************************/

	self.getName = () => "HTTP H.264";
	self.getMode = () => "media";

	self.getResolution = function() {
		return {
			// Разрешение видео или элемента
			"real_width": (__canvas.width || __canvas.offsetWidth),
			"real_height": (__canvas.height || __canvas.offsetHeight),
			"view_width": __canvas.offsetWidth,
			"view_height": __canvas.offsetHeight,
		};
	};

	self.ensureStream = function(state) {
		__state = state;
		__stop = false;
		__ensureMedia(false);
	};

	self.stopStream = function() {
		__stop = true;
		__ensuring = false;
		__wsForceClose();
		__setInfo(false, false, "");
	};

	var __ensureMedia = function(internal) {
		if (__ws === null && !__stop && (!__ensuring || internal)) {
			__ensuring = true;
			__setInactive();
			__setInfo(false, false, "");
			__logInfo("Starting Media ...");
			__ws = new WebSocket(`${tools.is_https ? "wss" : "ws"}://${location.host}/api/media/ws`);
			__ws.binaryType = "arraybuffer";
			__ws.onopen = __wsOpenHandler;
			__ws.onerror = __wsErrorHandler;
			__ws.onclose = __wsCloseHandler;
			__ws.onmessage = async (event) => {
				if (typeof event.data === "string") {
					__wsJsonHandler(JSON.parse(event.data));
				} else { // Binary
					await __wsBinHandler(event.data);
				}
			};
		}
	};

	var __wsOpenHandler = function(event) {
		__logInfo("Socket opened:", event);
		__missed_heartbeats = 0;
		__ping_timer = setInterval(__ping, 1000);
	};

	var __ping = function() {
		try {
			__missed_heartbeats += 1;
			if (__missed_heartbeats >= 5) {
				throw new Error("Too many missed heartbeats");
			}
			__ws.send(new Uint8Array([0]));

			if (__decoder && __decoder.state === "configured") {
				let online = !!(__state && __state.source.online);
				let info = `${__frames} fps dynamic`;
				__frames = 0;
				__setInfo(true, online, info);
			}
		} catch (ex) {
			__wsErrorHandler(ex.message);
		}
	};

	var __wsForceClose = function() {
		if (__ws) {
			__ws.onclose = null;
			__ws.close();
		}
		__wsCloseHandler(null);
		__setInactive();
	};

	var __wsErrorHandler = function(event) {
		__logInfo("Socket error:", event);
		__setInfo(false, false, event);
		__wsForceClose();
	};

	var __wsCloseHandler = function(event) {
		__logInfo("Socket closed:", event);
		if (__ping_timer) {
			clearInterval(__ping_timer);
			__ping_timer = null;
		}
		if (__decoder) {
			__decoder.close();
			__decoder = null;
		}
		__missed_heartbeats = 0;
		__frames = 0;
		__ws = null;
		if (!__stop) {
			setTimeout(() => __ensureMedia(true), 1000);
		}
	};

	var __wsJsonHandler = function(event) {
		if (event.event_type === "media") {
			__decoderCreate(event.event.video);
		}
	};

	var __wsBinHandler = async (data) => {
		let header = new Uint8Array(data.slice(0, 2));

		if (header[0] === 255) { // Pong
			__missed_heartbeats = 0;

		} else if (header[0] === 1 && __decoder !== null) { // Video frame
			let key = !!header[1];
			if (__decoder.state !== "configured") {
				if (!key) {
					return;
				}
				await __decoder.configure({"codec": __codec, "optimizeForLatency": true});
				__setActive();
			}

			let chunk = new EncodedVideoChunk({ // eslint-disable-line no-undef
				"timestamp": (performance.now() + performance.timeOrigin) * 1000,
				"type": (key ? "key" : "delta"),
				"data": data.slice(2),
			});
			await __decoder.decode(chunk);
		}
	};

	var __decoderCreate = function(formats) {
		__decoderDestroy();

		if (formats.h264 === undefined) {
			let msg = "No H.264 stream available on PiKVM";
			__setInfo(false, false, msg);
			__logInfo(msg);
			return;
		}
		if (!window.VideoDecoder) {
			let msg = "This browser can't handle direct H.264 stream";
			if (!tools.is_https) {
				msg = "Direct H.264 requires HTTPS";
			}
			__setInfo(false, false, msg);
			__logInfo(msg);
			return;
		}

		__decoder = new VideoDecoder({ // eslint-disable-line no-undef
			"output": (frame) => {
				try {
					if (__canvas.width !== frame.displayWidth || __canvas.height !== frame.displayHeight) {
						__canvas.width = frame.displayWidth;
						__canvas.height = frame.displayHeight;
					}
					__ctx.drawImage(frame, 0, 0);
					__frames += 1;
				} finally {
					frame.close();
				}
			},
			"error": (err) => __logInfo(err.message),
		});
		__codec = `avc1.${formats.h264.profile_level_id}`;

		__ws.send(JSON.stringify({
			"event_type": "start",
			"event": {"type": "video", "format": "h264"},
		}));
	};

	var __decoderDestroy = function() {
		if (__decoder !== null) {
			__decoder.close();
			__decoder = null;
			__codec = "";
		}
	};

	var __logInfo = (...args) => tools.info("Stream [Media]:", ...args);
}

MediaStreamer.is_videodecoder_available = function() {
	return !!window.VideoDecoder;
};
