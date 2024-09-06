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

import {JanusStreamer} from "./stream_janus.js";
import {MjpegStreamer} from "./stream_mjpeg.js";


export function Streamer() {
	var self = this;

	/************************************************************************/

	var __janus_enabled = null;
	var __streamer = null;

	var __state = null;
	var __resolution = {"width": 640, "height": 480};

	var __init__ = function() {
		__streamer = new MjpegStreamer(__setActive, __setInactive, __setInfo);

		$("stream-led").title = "Stream inactive";

		tools.slider.setParams($("stream-quality-slider"), 5, 100, 5, 80, function(value) {
			$("stream-quality-value").innerHTML = `${value}%`;
		});
		tools.slider.setOnUpDelayed($("stream-quality-slider"), 1000, (value) => __sendParam("quality", value));

		tools.slider.setParams($("stream-h264-bitrate-slider"), 25, 20000, 25, 5000, function(value) {
			$("stream-h264-bitrate-value").innerHTML = value;
		});
		tools.slider.setOnUpDelayed($("stream-h264-bitrate-slider"), 1000, (value) => __sendParam("h264_bitrate", value));

		tools.slider.setParams($("stream-h264-gop-slider"), 0, 60, 1, 30, function(value) {
			$("stream-h264-gop-value").innerHTML = value;
		});
		tools.slider.setOnUpDelayed($("stream-h264-gop-slider"), 1000, (value) => __sendParam("h264_gop", value));

		tools.slider.setParams($("stream-desired-fps-slider"), 0, 120, 1, 0, function(value) {
			$("stream-desired-fps-value").innerHTML = (value === 0 ? "Unlimited" : value);
		});
		tools.slider.setOnUpDelayed($("stream-desired-fps-slider"), 1000, (value) => __sendParam("desired_fps", value));

		$("stream-resolution-selector").onchange = (() => __sendParam("resolution", $("stream-resolution-selector").value));

		tools.radio.setOnClick("stream-mode-radio", __clickModeRadio, false);

		// Not getInt() because of radio is a string container.
		// Also don't reset Janus at class init.
		tools.radio.clickValue("stream-orient-radio", tools.storage.get("stream.orient", 0));
		tools.radio.setOnClick("stream-orient-radio", function() {
			if (__streamer.getMode() === "janus") { // Right now it's working only for H.264
				let orient = parseInt(tools.radio.getValue("stream-orient-radio"));
				tools.storage.setInt("stream.orient", orient);
				if (__streamer.getOrientation() != orient) {
					__resetStream();
				}
			}
		}, false);

		tools.slider.setParams($("stream-audio-volume-slider"), 0, 100, 1, 0, function(value) {
			$("stream-video").muted = !value;
			$("stream-video").volume = value / 100;
			$("stream-audio-volume-value").innerHTML = value + "%";
			if (__streamer.getMode() === "janus") {
				let allow_audio = !$("stream-video").muted;
				if (__streamer.isAudioAllowed() !== allow_audio) {
					__resetStream();
				}
			}
		});

		tools.el.setOnClick($("stream-screenshot-button"), __clickScreenshotButton);
		tools.el.setOnClick($("stream-reset-button"), __clickResetButton);

		$("stream-window").show_hook = () => __applyState(__state);
		$("stream-window").close_hook = () => __applyState(null);
	};

	/************************************************************************/

	self.getGeometry = function() {
		// Первоначально обновление геометрии считалось через ResizeObserver.
		// Но оно не ловило некоторые события, например в последовательности:
		//   - Находять в HD переходим в фулскрин
		//   - Меняем разрешение на маленькое
		//   - Убираем фулскрин
		//   - Переходим в HD
		//   - Видим нарушение пропорций
		// Так что теперь используются быстре рассчеты через offset*
		// вместо getBoundingClientRect().
		let res = __streamer.getResolution();
		let ratio = Math.min(res.view_width / res.real_width, res.view_height / res.real_height);
		return {
			"x": Math.round((res.view_width - ratio * res.real_width) / 2),
			"y": Math.round((res.view_height - ratio * res.real_height) / 2),
			"width": Math.round(ratio * res.real_width),
			"height": Math.round(ratio * res.real_height),
			"real_width": res.real_width,
			"real_height": res.real_height,
		};
	};

	self.setJanusEnabled = function(enabled) {
		let has_webrtc = JanusStreamer.is_webrtc_available();
		let has_h264 = JanusStreamer.is_h264_available();

		let set_enabled = function(imported) {
			tools.hidden.setVisible($("stream-message-no-webrtc"), enabled && !has_webrtc);
			tools.hidden.setVisible($("stream-message-no-h264"), enabled && !has_h264);
			__janus_enabled = (enabled && has_webrtc && imported); // Don't check has_h264 for sure
			tools.feature.setEnabled($("stream-mode"), __janus_enabled);
			tools.info(
				`Stream: Janus WebRTC state: enabled=${enabled},`
				+ ` webrtc=${has_webrtc}, h264=${has_h264}, imported=${imported}`
			);
			let mode = (__janus_enabled ? tools.storage.get("stream.mode", "janus") : "mjpeg");
			tools.radio.clickValue("stream-mode-radio", mode);
			if (!__janus_enabled) {
				tools.feature.setEnabled($("stream-audio"), false); // Enabling in stream_janus.js
			}
			self.setState(__state);
		};

		if (enabled && has_webrtc) {
			JanusStreamer.ensure_janus(set_enabled);
		} else {
			set_enabled(false);
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
			tools.feature.setEnabled($("stream-quality"), state.features.quality && (state.streamer === null || state.streamer.encoder.quality > 0));
			tools.feature.setEnabled($("stream-h264-bitrate"), state.features.h264 && __janus_enabled);
			tools.feature.setEnabled($("stream-h264-gop"), state.features.h264 && __janus_enabled);
			tools.feature.setEnabled($("stream-resolution"), state.features.resolution);

			if (state.streamer) {
				tools.el.setEnabled($("stream-quality-slider"), true);
				tools.slider.setValue($("stream-quality-slider"), state.streamer.encoder.quality);

				if (state.features.h264 && __janus_enabled) {
					__setLimitsAndValue($("stream-h264-bitrate-slider"), state.limits.h264_bitrate, state.streamer.h264.bitrate);
					tools.el.setEnabled($("stream-h264-bitrate-slider"), true);

					__setLimitsAndValue($("stream-h264-gop-slider"), state.limits.h264_gop, state.streamer.h264.gop);
					tools.el.setEnabled($("stream-h264-gop-slider"), true);
				}

				__setLimitsAndValue($("stream-desired-fps-slider"), state.limits.desired_fps, state.streamer.source.desired_fps);
				tools.el.setEnabled($("stream-desired-fps-slider"), true);

				let resolution_str = __makeStringResolution(state.streamer.source.resolution);
				if (__makeStringResolution(__resolution) !== resolution_str) {
					__resolution = state.streamer.source.resolution;
				}

				if (state.features.resolution) {
					let el = $("stream-resolution-selector");
					if (!state.limits.available_resolutions.includes(resolution_str)) {
						state.limits.available_resolutions.push(resolution_str);
					}
					tools.selector.setValues(el, state.limits.available_resolutions);
					tools.selector.setSelectedValue(el, resolution_str);
					tools.el.setEnabled(el, true);
				}

			} else {
				tools.el.setEnabled($("stream-quality-slider"), false);
				tools.el.setEnabled($("stream-h264-bitrate-slider"), false);
				tools.el.setEnabled($("stream-h264-gop-slider"), false);
				tools.el.setEnabled($("stream-desired-fps-slider"), false);
				tools.el.setEnabled($("stream-resolution-selector"), false);
			}

			__streamer.ensureStream(state.streamer);

		} else {
			__streamer.stopStream();
		}
	};

	var __setActive = function() {
		$("stream-led").className = "led-green";
		$("stream-led").title = "Stream is active";
	};

	var __setInactive = function() {
		$("stream-led").className = "led-gray";
		$("stream-led").title = "Stream inactive";
	};

	var __setInfo = function(is_active, online, text) {
		$("stream-box").classList.toggle("stream-box-offline", !online);
		let el_grab = document.querySelector("#stream-window-header .window-grab");
		let el_info = $("stream-info");
		let title = `${__streamer.getName()} &ndash; `;
		if (is_active) {
			if (!online) {
				title += "No signal / ";
			}
			title += __makeStringResolution(__resolution);
			if (text.length > 0) {
				title += " / " + text;
			}
		} else {
			if (text.length > 0) {
				title += text;
			} else {
				title += "Inactive";
			}
		}
		el_grab.innerHTML = el_info.innerHTML = title;
	};

	var __setLimitsAndValue = function(el, limits, value) {
		tools.slider.setRange(el, limits.min, limits.max);
		tools.slider.setValue(el, value);
	};

	var __resetStream = function(mode=null) {
		if (mode === null) {
			mode = __streamer.getMode();
		}
		__streamer.stopStream();
		if (mode === "janus") {
			__streamer = new JanusStreamer(__setActive, __setInactive, __setInfo,
				tools.storage.getInt("stream.orient", 0), !$("stream-video").muted);
			// Firefox doesn't support RTP orientation:
			//  - https://bugzilla.mozilla.org/show_bug.cgi?id=1316448
			tools.feature.setEnabled($("stream-orient"), !tools.browser.is_firefox);
		} else { // mjpeg
			__streamer = new MjpegStreamer(__setActive, __setInactive, __setInfo);
			tools.feature.setEnabled($("stream-orient"), false);
			tools.feature.setEnabled($("stream-audio"), false); // Enabling in stream_janus.js
		}
		if (wm.isWindowVisible($("stream-window"))) {
			__streamer.ensureStream(__state ? __state.streamer : null);
		}
	};

	var __clickModeRadio = function() {
		let mode = tools.radio.getValue("stream-mode-radio");
		tools.storage.set("stream.mode", mode);
		if (mode !== __streamer.getMode()) {
			tools.hidden.setVisible($("stream-image"), (mode !== "janus"));
			tools.hidden.setVisible($("stream-video"), (mode === "janus"));
			__resetStream(mode);
		}
	};

	var __clickScreenshotButton = function() {
		let el = document.createElement("a");
		el.href = "/api/streamer/snapshot";
		el.target = "_blank";
		document.body.appendChild(el);
		el.click();
		setTimeout(() => document.body.removeChild(el), 0);
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset stream?").then(function (ok) {
			if (ok) {
				__resetStream();
				tools.httpPost("/api/streamer/reset", function(http) {
					if (http.status !== 200) {
						wm.error("Can't reset stream:<br>", http.responseText);
					}
				});
			}
		});
	};

	var __sendParam = function(name, value) {
		tools.httpPost(`/api/streamer/set_params?${name}=${value}`, function(http) {
			if (http.status !== 200) {
				wm.error("Can't configure stream:<br>", http.responseText);
			}
		});
	};

	var __makeStringResolution = function(resolution) {
		return `${resolution.width}x${resolution.height}`;
	};

	__init__();
}
