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


/*function _JanusStreamer(set_active_callback, set_inactive_callback, set_info_callback) {
	var self = this;

	var __janus = null;
	var __handle = null;
	var __bitrate_timer = null;

	self.getResolution = function() {
		let el_video = $("stream-video");
		return {
			real_width: el_video.videoWidth,
			real_height: el_video.videoHeight,
			view_width: el_video.offsetWidth,
			view_height: el_video.offsetHeight,
		};
	};
};*/

function _MjpegStreamer(set_active_callback, set_inactive_callback, set_info_callback) {
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
				__setActive();
				__stopChecking();
			} else {
				__ensureChecking();
			}
		} else {
			__stopChecking();
			__setInactive();
		}
	};

	self.stopStream = function() {
		self.ensureStream(null);
		let blank = "/share/png/blank-stream.png";
		if (!String.prototype.endsWith.call($("stream-image").src, blank)) {
			$("stream-image").src = blank;
		}
	};

	var __setActive = function() {
		let old_fps = __fps;
		__fps = __state.stream.clients_stat[__id].fps;
		if (old_fps < 0) {
			tools.info("Stream [MJPEG]: Active");
			set_active_callback();
		}
		set_info_callback(true, __state.source.online, `${__fps} fps`);
	};

	var __setInactive = function() {
		let old_fps = __fps;
		__key = tools.makeId();
		__id = "";
		__fps = -1;
		__state = null;
		if (old_fps >= 0) {
			tools.info("Stream [MJPEG]: Inactive");
			set_inactive_callback();
			set_info_callback(false, false, "");
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
			tools.info("Stream [MJPEG]: Found acceptable stream_client cookie:", stream_client);
			__id = stream_client.slice(stream_client.indexOf("/") + 1);
		}
	};

	var __checkStream = function() {
		__findId();

		if (__id.legnth > 0 && __id in __state.stream.clients_stat) {
			__setActive();
			__stopChecking();

		} else if (__id.length > 0 && __timer_retries >= 0) {
			__timer_retries -= 1;

		} else {
			__setInactive();
			__stopChecking();

			let path = `/streamer/stream?key=${__key}`;
			if (tools.browser.is_safari || tools.browser.is_ios) {
				// uStreamer fix for WebKit
				tools.info("Stream [MJPEG]: Using dual_final_frames=1 to fix WebKit bugs");
				path += "&dual_final_frames=1";
			} else if (tools.browser.is_chrome || tools.browser.is_blink) {
				// uStreamer fix for Blink https://bugs.chromium.org/p/chromium/issues/detail?id=527446
				tools.info("Stream [MJPEG]: Using advance_headers=1 to fix Blink bugs");
				path += "&advance_headers=1";
			}

			tools.info("Stream [MJPEG]: Refreshing ...");
			$("stream-image").src = path;
		}
	};
}

export function Streamer() {
	var self = this;

	/************************************************************************/

	var __janus_enabled = null;
	var __mjpeg = null;

	var __state = null;
	var __resolution = {width: 640, height: 480};

	var __init__ = function() {
		__mjpeg = new _MjpegStreamer(__setActive, __setInactive, __setInfo);

		$("stream-led").title = "Stream inactive";

		tools.sliderSetParams($("stream-quality-slider"), 5, 100, 5, 80);
		tools.sliderSetOnUp($("stream-quality-slider"), 1000, __updateQualityValue, (value) => __sendParam("quality", value));

		tools.sliderSetParams($("stream-desired-fps-slider"), 0, 120, 1, 0);
		tools.sliderSetOnUp($("stream-desired-fps-slider"), 1000, __updateDesiredFpsValue, (value) => __sendParam("desired_fps", value));

		$("stream-resolution-selector").onchange = (() => __sendParam("resolution", $("stream-resolution-selector").value));

		tools.setOnClick($("stream-screenshot-button"), __clickScreenshotButton);
		tools.setOnClick($("stream-reset-button"), __clickResetButton);

		$("stream-window").show_hook = () => __applyState(__state);
		$("stream-window").close_hook = () => __applyState(null);
	};

	/************************************************************************/

	self.getResolution = function() {
		return __mjpeg.getResolution();
	};

	self.setJanusEnabled = function(enabled) {
		let supported = !!window.RTCPeerConnection;
		let set_enabled = function() {
			__janus_enabled = (enabled && supported && _Janus !== null);
			tools.info(`Stream: Janus WebRTC state: enabled=${enabled}, supported=${supported}, imported=${!!_Janus}`);
			self.setState(__state);
		};
		if (enabled && supported) {
			if (_Janus === null) {
				import("./janus.js").then((module) => {
					_Janus = module.Janus;
					set_enabled();
				}).catch((err) => {
					tools.error("Can't import Janus module:", err);
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
			tools.featureSetEnabled($("stream-resolution"), state.features.resolution);

			if (state.streamer) {
				if (!$("stream-quality-slider").activated) {
					wm.setElementEnabled($("stream-quality-slider"), true);
					if ($("stream-quality-slider").value !== state.streamer.encoder.quality) {
						$("stream-quality-slider").value = state.streamer.encoder.quality;
						__updateQualityValue(state.streamer.encoder.quality);
					}
				}

				if (!$("stream-desired-fps-slider").activated) {
					$("stream-desired-fps-slider").min = state.limits.desired_fps.min;
					$("stream-desired-fps-slider").max = state.limits.desired_fps.max;
					wm.setElementEnabled($("stream-desired-fps-slider"), true);
					if ($("stream-desired-fps-slider").value !== state.streamer.source.desired_fps) {
						$("stream-desired-fps-slider").value = state.streamer.source.desired_fps;
						__updateDesiredFpsValue(state.streamer.source.desired_fps);
					}
				}

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
			}

			__mjpeg.ensureStream(state.streamer);

		} else {
			__mjpeg.stopStream();
		}
	};

	var __setActive = function() {
		$("stream-led").className = "led-green";
		$("stream-led").title = "Stream is active";
		wm.setElementEnabled($("stream-screenshot-button"), true);
		wm.setElementEnabled($("stream-reset-button"), true);
		$("stream-quality-slider").activated = false;
		$("stream-desired-fps-slider").activated = false;
	};

	var __setInactive = function() {
		$("stream-led").className = "led-gray";
		$("stream-led").title = "Stream inactive";
		wm.setElementEnabled($("stream-screenshot-button"), false);
		wm.setElementEnabled($("stream-reset-button"), false);
		wm.setElementEnabled($("stream-quality-slider"), false);
		wm.setElementEnabled($("stream-desired-fps-slider"), false);
		wm.setElementEnabled($("stream-resolution-selector"), false);
	};

	var __setInfo = function(is_active, online, text) {
		$("stream-box").classList.toggle("stream-box-offline", !online);
		let el_grab = document.querySelector("#stream-window-header .window-grab");
		let el_info = $("stream-info");
		if (is_active) {
			let title = "Stream &ndash; ";
			if (!online) {
				title += "no signal / ";
			}
			title += __makeStringResolution(__resolution);
			if (text.length > 0) {
				title += " / " + text;
			}
			el_grab.innerHTML = el_info.innerHTML = title;
		} else {
			el_grab.innerHTML = el_info.innerHTML = "Stream &ndash; inactive";
		}
	};

	var __updateQualityValue = function(value) {
		$("stream-quality-value").innerHTML = `${value}%`;
	};

	var __updateDesiredFpsValue = function(value) {
		$("stream-desired-fps-value").innerHTML = (value === 0 ? "Unlimited" : value);
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
