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


var _Janus = null;


export function JanusStreamer(__setActive, __setInactive, __setInfo, __allow_audio) {
	var self = this;

	var __stop = false;
	var __ensuring = false;

	var __janus = null;
	var __handle = null;

	var __retry_ensure_timeout = null;
	var __retry_emsg_timeout = null;
	var __info_interval = null;

	var __state = null;
	var __frames = 0;

	self.isAudioAllowed = () => __allow_audio;

	self.getName = () => (__allow_audio ? "H.264 + Audio" : "H.264");
	self.getMode = () => "janus";

	self.getResolution = function() {
		let el = $("stream-video");
		return {
			// Разрешение видео или элемента
			"real_width": (el.videoWidth || el.offsetWidth),
			"real_height": (el.videoHeight || el.offsetHeight),
			"view_width": el.offsetWidth,
			"view_height": el.offsetHeight,
		};
	};

	self.ensureStream = function(state) {
		__state = state;
		__stop = false;
		__ensureJanus(false);
	};

	self.stopStream = function() {
		__stop = true;
		__destroyJanus();
	};

	var __ensureJanus = function(internal) {
		if (__janus === null && !__stop && (!__ensuring || internal)) {
			__setInactive();
			__setInfo(false, false, "");
			__ensuring = true;
			__logInfo("Starting Janus ...");
			__janus = new _Janus({
				"server": `${tools.is_https ? "wss" : "ws"}://${location.host}/janus/ws`,
				"ipv6": true,
				"destroyOnUnload": false,
				"success": __attachJanus,
				"error": function(error) {
					__logError(error);
					__setInfo(false, false, error);
					__finishJanus();
				},
			});
		}
	};

	var __finishJanus = function() {
		if (__stop) {
			if (__retry_ensure_timeout !== null) {
				clearTimeout(__retry_ensure_timeout);
				__retry_ensure_timeout = null;
			}
			__ensuring = false;
		} else {
			if (__retry_ensure_timeout === null) {
				__retry_ensure_timeout = setTimeout(function() {
					__retry_ensure_timeout = null;
					__ensureJanus(true);
				}, 5000);
			}
		}
		__stopRetryEmsgInterval();
		__stopInfoInterval();
		__handle = null;
		__janus = null;
		__setInactive();
		if (__stop) {
			__setInfo(false, false, "");
		}
	};

	var __destroyJanus = function() {
		if (__handle && __handle.webrtcStuff && __handle.webrtcStuff.remoteStream) {
			for (let track of __handle.webrtcStuff.remoteStream.getTracks()) {
				track.stop();
				__handle.webrtcStuff.remoteStream.removeTrack(track);
			}
			__handle.webrtcStuff.remoteStream = null;
		}
		$("stream-video").srcObject = null;
		if (__janus !== null) {
			__janus.destroy();
		}
		__finishJanus();
	};

	var __attachJanus = function() {
		if (__janus === null) {
			return;
		}
		__janus.attach({
			"plugin": "janus.plugin.ustreamer",
			"opaqueId": "oid-" + _Janus.randomString(12),

			"success": function(handle) {
				__handle = handle;
				__logInfo("uStreamer attached:", handle.getPlugin(), handle.getId());
				__sendWatch();
			},

			"error": function(error) {
				__logError("Can't attach uStreamer: ", error);
				__setInfo(false, false, error);
				__destroyJanus();
			},

			"iceState": function(state) {
				__logInfo("ICE state changed to", state);
				// Если раскомментировать, то он начнет дрючить соединение,
				// так как каллбек вызывает сильно после завершения работы
				/*if (state === "disconnected") {
					__destroyJanus();
				}*/
			},

			"webrtcState": function(up) {
				__logInfo("Janus says our WebRTC PeerConnection is", (up ? "up" : "down"), "now");
				if (up) {
					__sendKeyRequired();
				}
			},

			"onmessage": function(msg, jsep) {
				__stopRetryEmsgInterval();

				if (msg.result) {
					__logInfo("Got uStreamer result message:", msg.result.status); // starting, started, stopped
					if (msg.result.status === "started") {
						__setActive();
						__setInfo(false, false, "");
					} else if (msg.result.status === "stopped") {
						__setInactive();
						__setInfo(false, false, "");
					} else if (msg.result.status === "features") {
						tools.feature.setEnabled($("stream-audio"), msg.result.features.audio);
					}
				} else if (msg.error_code || msg.error) {
					__logError("Got uStreamer error message:", msg.error_code, "-", msg.error);
					__setInfo(false, false, msg.error);
					if (__retry_emsg_timeout === null) {
						__retry_emsg_timeout = setTimeout(function() {
							if (!__stop) {
								__sendStop();
								__sendWatch();
							}
							__retry_emsg_timeout = null;
						}, 2000);
					}
					return;
				} else {
					__logInfo("Got uStreamer other message:", msg);
				}

				if (jsep) {
					__logInfo("Handling SDP:", jsep);
					__handle.createAnswer({
						"jsep": jsep,
						"media": {"audioSend": false, "videoSend": false, "data": false},

						"success": function(jsep) {
							__logInfo("Got SDP:", jsep);
							__sendStart(jsep);
						},

						"error": function(error) {
							__logInfo("Error on SDP handling:", error);
							__setInfo(false, false, error);
							//__destroyJanus();
						},
					});
				}
			},

			"onremotestream": function(stream) {
				__logInfo("Got a remote stream:", stream);
				_Janus.attachMediaStream($("stream-video"), stream);
				__sendKeyRequired();
				__startInfoInterval();
				// FIXME: Задержка уменьшается, но начинаются заикания на кейфреймах.
				//   - https://github.com/Glimesh/janus-ftl-plugin/issues/101
				/*if (__handle && __handle.webrtcStuff && __handle.webrtcStuff.pc) {
					for (let receiver of __handle.webrtcStuff.pc.getReceivers()) {
						if (receiver.track && receiver.track.kind === "video" && receiver.playoutDelayHint !== undefined) {
							receiver.playoutDelayHint = 0;
						}
					}
				}*/
			},

			"oncleanup": function() {
				__logInfo("Got a cleanup notification");
				__stopInfoInterval();
			},
		});
	};

	var __startInfoInterval = function() {
		__stopInfoInterval();
		__setActive();
		__updateInfo();
		__info_interval = setInterval(__updateInfo, 1000);
	};

	var __stopInfoInterval = function() {
		if (__info_interval !== null) {
			clearInterval(__info_interval);
		}
		__info_interval = null;
	};

	var __stopRetryEmsgInterval = function() {
		if (__retry_emsg_timeout !== null) {
			clearTimeout(__retry_emsg_timeout);
			__retry_emsg_timeout = null;
		}
	};

	var __updateInfo = function() {
		if (__handle !== null) {
			let online = !!(__state && __state.source && __state.source.online);
			let info = "";
			if (__handle !== null) {
				// https://wiki.whatwg.org/wiki/Video_Metrics
				let frames = null;
				let el = $("stream-video");
				if (el.webkitDecodedFrameCount !== undefined) {
					frames = el.webkitDecodedFrameCount;
				} else if (el.mozPaintedFrames !== undefined) {
					frames = el.mozPaintedFrames;
				}
				if (frames !== null) {
					info = `${Math.max(0, frames - __frames)} fps dynamic`;
					__frames = frames;
				} else {
					info = `${__handle.getBitrate()}`.replace("kbits/sec", "kbps");
				}
			}
			__setInfo(true, online, info);
		}
	};

	var __sendWatch = function() {
		if (__handle) {
			__logInfo(`Sending WATCH(audio=${__allow_audio}) + FEATURES ...`);
			__handle.send({"message": {"request": "features"}});
			__handle.send({"message": {"request": "watch", "params": {"audio": __allow_audio}}});
		}
	};

	var __sendStart = function(jsep) {
		if (__handle) {
			__logInfo("Sending START ...");
			__handle.send({"message": {"request": "start"}, "jsep": jsep});
		}
	};

	var __sendKeyRequired = function() {
		/*if (__handle) {
			// На этом шаге мы говорим что стрим пошел и надо запросить кейфрейм
			__logInfo("Sending KEY_REQUIRED ...");
			__handle.send({message: {request: "key_required"}});
		}*/
	};

	var __sendStop = function() {
		__stopInfoInterval();
		if (__handle) {
			__logInfo("Sending STOP ...");
			__handle.send({"message": {"request": "stop"}});
			__handle.hangup();
		}
	};

	var __logInfo = (...args) => tools.info("Stream [Janus]:", ...args);
	var __logError = (...args) => tools.error("Stream [Janus]:", ...args);
}

JanusStreamer.ensure_janus = function(callback) {
	if (_Janus === null) {
		import("./janus.js").then((module) => {
			module.Janus.init({
				"debug": "all",
				"callback": function() {
					_Janus = module.Janus;
					callback(true);
				},
			});
		}).catch((err) => {
			tools.error("Stream: Can't import Janus module:", err);
			callback(false);
		});
	} else {
		callback(true);
	}
};

JanusStreamer.is_webrtc_available = function() {
	return !!window.RTCPeerConnection;
};

JanusStreamer.is_h264_available = function() {
	let ok = true;
	if ($("stream-video").canPlayType) {
		ok = $("stream-video").canPlayType("video/mp4; codecs=\"avc1.42E01F\"");
	}
	return ok;
};
