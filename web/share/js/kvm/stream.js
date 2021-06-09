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


var _Janus = null;


function _JanusStreamer(__setActive, __setInactive, __setInfo) {
	var self = this;

	var __stop = false;
	var __ensuring = false;

	var __janus = null;
	var __handle = null;

	var __retry_ensure_timeout = null;
	var __retry_emsg_timeout = null;
	var __info_interval = null;

	var __state = null;

	self.getResolution = function() {
		let el_video = $("stream-video");
		return {
			real_width: el_video.videoWidth,
			real_height: el_video.videoHeight,
			view_width: el_video.offsetWidth,
			view_height: el_video.offsetHeight,
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
				server: `${tools.https ? "wss" : "ws"}://${location.host}/janus/ws`,
				ipv6: true,
				destroyOnUnload: false,
				success: __attachJanus,
				error: function(error) {
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
		if (__handle.webrtcStuff.remoteStream !== null) {
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
			plugin: "janus.plugin.ustreamer",
			opaqueId: "oid-" + _Janus.randomString(12),

			success: function(handle) {
				__handle = handle;
				__logInfo("uStreamer attached:", handle.getPlugin(), handle.getId());
				__sendWatch();
			},

			error: function(error) {
				__logError("Can't attach uStreamer: ", error);
				__setInfo(false, false, error);
				__destroyJanus();
			},

			iceState: function(state) {
				__logInfo("ICE state changed to", state);
				// Если раскомментировать, то он начнет дрючить соединение,
				// так как каллбек вызывает сильно после завершения работы
				/*if (state === "disconnected") {
					__destroyJanus();
				}*/
			},

			webrtcState: function(up) {
				__logInfo("Janus says our WebRTC PeerConnection is", (up ? "up" : "down"), "now");
			},

			onmessage: function(msg, jsep) {
				__stopRetryEmsgInterval();

				if (msg.result) {
					__logInfo("Got uStreamer result message:", msg.result.status); // starting, started, stopped
					if (msg.result.status === "started") {
						__setActive();
						__setInfo(false, false, "");
					} else if (msg.result.status === "stopped") {
						__setInactive();
						__setInfo(false, false, "");
					}
				} else if (msg.error_code || msg.error) {
					__logError("Got uStreamer error message:", msg.error_code, "-", msg.error);
					__setInfo(false, false, (msg.error_code === 503 ? "Waiting for keyframe ..." : msg.error));
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
						jsep: jsep,
						media: {audioSend: false, videoSend: false, data: false},

						success: function(jsep) {
							__logInfo("Got SDP:", jsep);
							__sendStart(jsep);
						},

						error: function(error) {
							__logInfo("Error on SDP handling:", error);
							__setInfo(false, false, error);
							//__destroyJanus();
						},
					});
				}
			},

			onremotestream: function(stream) {
				__logInfo("Got a remote stream:", stream);
				_Janus.attachMediaStream($("stream-video"), stream);
				__startInfoInterval();
			},

			oncleanup: function() {
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
			let bitrate = (__handle !== null ? __handle.getBitrate() : "");
			__setInfo(true, online, bitrate);
		}
	};

	var __sendWatch = function() {
		if (__handle) {
			__logInfo("Sending WATCH ...");
			__handle.send({message: {request: "watch"}});
		}
	};

	var __sendStart = function(jsep) {
		if (__handle) {
			__logInfo("Sending START ...");
			__handle.send({message: {request: "start"}, jsep: jsep});
		}
	};

	var __sendStop = function() {
		__stopInfoInterval();
		if (__handle) {
			__logInfo("Sending STOP ...");
			__handle.send({message: {request: "stop"}});
			__handle.hangup();
		}
	};

	var __logInfo = (...args) => tools.info("Stream [Janus]:", ...args);
	var __logError = (...args) => tools.error("Stream [Janus]:", ...args);
}

function _MjpegStreamer(__setActive, __setInactive, __setInfo) {
	var self = this;

	/************************************************************************/

	var __key = tools.makeId();
	var __id = "";
	var __fps = -1;
	var __state = null;

	var __timer = null;
	var __timer_retries = 0;

	/************************************************************************/

	self.getResolution = function() {
		let el_image = $("stream-image");
		return {
			real_width: el_image.naturalWidth,
			real_height: el_image.naturalHeight,
			view_width: el_image.offsetWidth,
			view_height: el_image.offsetHeight,
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
		__setInfo(true, __state.source.online, `${__fps} fps`);
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
		let stream_client = tools.getCookie("stream_client");
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

export function Streamer() {
	var self = this;

	/************************************************************************/

	var __janus_enabled = null;
	var __streamer = null;

	var __state = null;
	var __resolution = {width: 640, height: 480};

	var __init__ = function() {
		__streamer = new _MjpegStreamer(__setActive, __setInactive, __setInfo);

		$("stream-led").title = "Stream inactive";

		tools.sliderSetParams($("stream-quality-slider"), 5, 100, 5, 80);
		tools.sliderSetOnUp($("stream-quality-slider"), 1000, __updateQualityValue, (value) => __sendParam("quality", value));

		tools.sliderSetParams($("stream-h264-bitrate-slider"), 100, 16000, 100, 5000);
		tools.sliderSetOnUp($("stream-h264-bitrate-slider"), 1000, __updateH264BitrateValue, (value) => __sendParam("h264_bitrate", value));

		tools.sliderSetParams($("stream-desired-fps-slider"), 0, 120, 1, 0);
		tools.sliderSetOnUp($("stream-desired-fps-slider"), 1000, __updateDesiredFpsValue, (value) => __sendParam("desired_fps", value));

		$("stream-resolution-selector").onchange = (() => __sendParam("resolution", $("stream-resolution-selector").value));

		tools.radioSetOnClick("stream-mode-radio", __clickModeRadio);

		tools.setOnClick($("stream-screenshot-button"), __clickScreenshotButton);
		tools.setOnClick($("stream-reset-button"), __clickResetButton);

		$("stream-window").show_hook = () => __applyState(__state);
		$("stream-window").close_hook = () => __applyState(null);
	};

	/************************************************************************/

	self.getResolution = function() {
		return __streamer.getResolution();
	};

	self.setJanusEnabled = function(enabled) {
		let supported = !!window.RTCPeerConnection;
		let set_enabled = function() {
			__janus_enabled = (enabled && supported && _Janus !== null);
			tools.featureSetEnabled($("stream-mode"), __janus_enabled);
			tools.info(`Stream: Janus WebRTC state: enabled=${enabled}, supported=${supported}, imported=${!!_Janus}`);
			self.setState(__state);
		};
		if (enabled && supported) {
			if (_Janus === null) {
				import("./janus.js").then((module) => {
					module.Janus.init({
						debug: "all",
						callback: function() {
							_Janus = module.Janus;
							set_enabled();
						},
					});
				}).catch((err) => {
					tools.error("Stream: Can't import Janus module:", err);
					set_enabled();
				});
			} else {
				set_enabled();
			}
		} else {
			set_enabled();
		}
	};

	self.setState = function(state) {
		__state = state;
		if (__janus_enabled !== null) {
			__applyState(wm.isWindowVisible($("stream-window")) ? __state : null);
		}
	};

	var __applyState = function(state) {
		if (state) {
			tools.featureSetEnabled($("stream-quality"), state.features.quality && (state.streamer === null || state.streamer.encoder.quality > 0));
			tools.featureSetEnabled($("stream-h264-bitrate"), state.features.h264 && __janus_enabled);
			tools.featureSetEnabled($("stream-resolution"), state.features.resolution);

			if (state.streamer) {
				wm.setElementEnabled($("stream-quality-slider"), true);
				__setIfChanged($("stream-quality-slider"), state.streamer.encoder.quality, __updateQualityValue);

				if (state.features.h264 && __janus_enabled) {
					__setMinMax($("stream-h264-bitrate-slider"), state.limits.h264_bitrate);
					wm.setElementEnabled($("stream-h264-bitrate-slider"), true);
					__setIfChanged($("stream-h264-bitrate-slider"), state.streamer.h264.bitrate, __updateH264BitrateValue);
				}

				__setMinMax($("stream-desired-fps-slider"), state.limits.desired_fps);
				wm.setElementEnabled($("stream-desired-fps-slider"), true);
				__setIfChanged($("stream-desired-fps-slider"), state.streamer.source.desired_fps, __updateDesiredFpsValue);

				let resolution_str = __makeStringResolution(state.streamer.source.resolution);
				if (__makeStringResolution(__resolution) !== resolution_str) {
					__resolution = state.streamer.source.resolution;
				}

				if (state.features.resolution) {
					if ($("stream-resolution-selector").resolutions !== state.limits.available_resolutions) {
						let resolutions_html = "";
						for (let variant of state.limits.available_resolutions) {
							resolutions_html += `<option value="${variant}">${variant}</option>`;
						}
						if (!state.limits.available_resolutions.includes(resolution_str)) {
							resolutions_html += `<option value="${resolution_str}">${resolution_str}</option>`;
						}
						$("stream-resolution-selector").innerHTML = resolutions_html;
						$("stream-resolution-selector").resolutions = state.limits.available_resolutions;
					}
					document.querySelector(`#stream-resolution-selector [value="${resolution_str}"]`).selected = true;
					wm.setElementEnabled($("stream-resolution-selector"), true);
				}

			} else {
				wm.setElementEnabled($("stream-quality-slider"), false);
				wm.setElementEnabled($("stream-h264-bitrate-slider"), false);
				wm.setElementEnabled($("stream-desired-fps-slider"), false);
				wm.setElementEnabled($("stream-resolution-selector"), false);
			}

			__streamer.ensureStream(state.streamer);

		} else {
			__streamer.stopStream();
		}
	};

	var __setActive = function() {
		$("stream-led").className = "led-green";
		$("stream-led").title = "Stream is active";
		wm.setElementEnabled($("stream-screenshot-button"), true);
		wm.setElementEnabled($("stream-reset-button"), true);
	};

	var __setInactive = function() {
		$("stream-led").className = "led-gray";
		$("stream-led").title = "Stream inactive";
		wm.setElementEnabled($("stream-screenshot-button"), false);
		wm.setElementEnabled($("stream-reset-button"), false);
	};

	var __setInfo = function(is_active, online, text) {
		$("stream-box").classList.toggle("stream-box-offline", !online);
		let el_grab = document.querySelector("#stream-window-header .window-grab");
		let el_info = $("stream-info");
		let title = "Stream &ndash; ";
		if (is_active) {
			if (!online) {
				title += "no signal / ";
			}
			title += __makeStringResolution(__resolution);
			if (text.length > 0) {
				title += " / " + text;
			}
		} else {
			if (text.length > 0) {
				title += text;
			} else {
				title += "inactive";
			}
		}
		el_grab.innerHTML = el_info.innerHTML = title;
	};

	var __setMinMax = function(el, limits) {
		el.min = limits.min;
		el.max = limits.max;
	};

	var __setIfChanged = function(el, value, callback) {
		if (el.value !== value) {
			el.value = value;
			callback(value);
		}
	};

	var __updateQualityValue = function(value) {
		$("stream-quality-value").innerHTML = `${value}%`;
	};

	var __updateDesiredFpsValue = function(value) {
		$("stream-desired-fps-value").innerHTML = (value === 0 ? "Unlimited" : value);
	};

	var __updateH264BitrateValue = function(value) {
		$("stream-h264-bitrate-value").innerHTML = value;
	};

	var __clickModeRadio = function() {
		if (_Janus !== null) {
			let mode = tools.radioGetValue("stream-mode-radio");
			setTimeout(() => tools.radioSetValue("stream-mode-radio", mode), 100);
			tools.hiddenSetVisible($("stream-image"), (mode === "mjpeg"));
			tools.hiddenSetVisible($("stream-video"), (mode !== "mjpeg"));
			if (mode === "mjpeg") {
				__streamer.stopStream();
				__streamer = new _MjpegStreamer(__setActive, __setInactive, __setInfo);
			} else { // janus
				__streamer.stopStream();
				__streamer = new _JanusStreamer(__setActive, __setInactive, __setInfo);
			}
			if (wm.isWindowVisible($("stream-window"))) {
				__streamer.ensureStream(__state);
			}
		}
	};

	var __clickScreenshotButton = function() {
		let el_a = document.createElement("a");
		el_a.href = "/api/streamer/snapshot?allow_offline=1";
		el_a.target = "_blank";
		document.body.appendChild(el_a);
		el_a.click();
		setTimeout(() => document.body.removeChild(el_a), 0);
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset stream?").then(function (ok) {
			if (ok) {
				if (wm.isWindowVisible($("stream-window"))) {
					__streamer.stopStream();
					__streamer.ensureStream(__state);
				}

				let http = tools.makeRequest("POST", "/api/streamer/reset", function() {
					if (http.readyState === 4) {
						if (http.status !== 200) {
							wm.error("Can't reset stream:<br>", http.responseText);
						}
					}
				});
			}
		});
	};

	var __sendParam = function(name, value) {
		let http = tools.makeRequest("POST", `/api/streamer/set_params?${name}=${value}`, function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					wm.error("Can't configure stream:<br>", http.responseText);
				}
			}
		});
	};

	var __makeStringResolution = function(resolution) {
		return `${resolution.width}x${resolution.height}`;
	};

	__init__();
}
