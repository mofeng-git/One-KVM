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


export function MediaStreamer(__setActive, __setInactive, __setInfo, __orient) {
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
	var __fps_accum = 0;

	/************************************************************************/

	self.getOrientation = () => __orient;
	self.getName = () => "Direct H.264";
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
			__ws = new WebSocket(tools.makeWsUrl("api/media/ws"));
			__ws.binaryType = "arraybuffer";
			__ws.onopen = __wsOpenHandler;
			__ws.onerror = __wsErrorHandler;
			__ws.onclose = __wsCloseHandler;
			__ws.onmessage = async (event) => {
				try {
					if (typeof event.data === "string") {
						event = JSON.parse(event.data);
						__wsJsonHandler(event.event_type, event.event);
					} else { // Binary
						await __wsBinHandler(event.data);
					}
				} catch (ex) {
					__wsErrorHandler(ex);
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
				let info = `${__fps_accum} fps dynamic`;
				__fps_accum = 0;
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
		__closeDecoder();
		__missed_heartbeats = 0;
		__fps_accum = 0;
		__ws = null;
		if (!__stop) {
			setTimeout(() => __ensureMedia(true), 1000);
		}
	};

	var __wsJsonHandler = function(event_type, event) {
		if (event_type === "media") {
			__setupCodec(event.video);
		}
	};

	var __setupCodec = function(formats) {
		__closeDecoder();
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
		__codec = `avc1.${formats.h264.profile_level_id}`;
		__ws.send(JSON.stringify({
			"event_type": "start",
			"event": {"type": "video", "format": "h264"},
		}));
	};

	var __wsBinHandler = async (data) => {
		let header = new Uint8Array(data.slice(0, 2));
		if (header[0] === 255) { // Pong
			__missed_heartbeats = 0;
		} else if (header[0] === 1) { // Video frame
			let key = !!header[1];
			if (await __ensureDecoder(key)) {
				await __processFrame(key, data.slice(2));
			}
		}
	};

	var __ensureDecoder = async (key) => {
		if (__codec === "") {
			return false;
		}
		if (__decoder === null || __decoder.state === "closed") {
			let started = (__codec !== "");
			let codec = __codec;
			__closeDecoder();
			__codec = codec;
			__decoder = new VideoDecoder({ // eslint-disable-line no-undef
				"output": __drawFrame,
				"error": (err) => __logInfo(err.message),
			});
			if (started) {
				__ws.send(new Uint8Array([0]));
			}
		}
		if (__decoder.state !== "configured") {
			if (!key) {
				return false;
			}
			await __decoder.configure({"codec": __codec, "optimizeForLatency": true});
		}
		if (__decoder.state === "configured") {
			__setActive();
			return true;
		}
		return false;
	};

	var __processFrame = async (key, raw) => {
		let chunk = new EncodedVideoChunk({ // eslint-disable-line no-undef
			"timestamp": (performance.now() + performance.timeOrigin) * 1000,
			"type": (key ? "key" : "delta"),
			"data": raw,
		});
		await __decoder.decode(chunk);
	};

	var __closeDecoder = function() {
		if (__decoder !== null) {
			try {
				__decoder.close();
			} catch { // eslint-disable-line no-empty
			} finally {
				__decoder = null;
				__codec = "";
			}
		}
	};

	var __drawFrame = function(frame) {
		try {
			let width = frame.displayWidth;
			let height = frame.displayHeight;
			switch (__orient) {
				case 90:
				case 270:
					width = frame.displayHeight;
					height = frame.displayWidth;
			}

			if (__canvas.width !== width || __canvas.height !== height) {
				__canvas.width = width;
				__canvas.height = height;
			}

			if (__orient === 0) {
				__ctx.drawImage(frame, 0, 0);
			} else {
				__ctx.save();
				try {
					switch(__orient) {
						case 90: __ctx.translate(0, height); __ctx.rotate(-Math.PI / 2); break;
						case 180: __ctx.translate(width, height); __ctx.rotate(-Math.PI); break;
						case 270: __ctx.translate(width, 0); __ctx.rotate(Math.PI / 2); break;
					}
					__ctx.drawImage(frame, 0, 0);
				} finally {
					__ctx.restore();
				}
			}

			__fps_accum += 1;
		} finally {
			frame.close();
		}
	};

	var __logInfo = (...args) => tools.info("Stream [Media]:", ...args);
}

MediaStreamer.is_videodecoder_available = function() {
	return !!window.VideoDecoder;
};
