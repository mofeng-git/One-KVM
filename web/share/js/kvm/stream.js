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
import {MediaStreamer} from "./stream_media.js";
import {MjpegStreamer} from "./stream_mjpeg.js";


export function Streamer() {
	var self = this;

	/************************************************************************/

	var __janus_imported = null;
	var __streamer = null;

	var __state = null;
	var __res = {"width": 640, "height": 480};

	var __init__ = function() {
		__streamer = new MjpegStreamer(__setActive, __setInactive, __setInfo);

		$("stream-led").title = "Stream inactive";

		tools.slider.setParams($("stream-quality-slider"), 5, 100, 5, 80, function(value) {
			$("stream-quality-value").innerText = `${value}%`;
		});
		tools.slider.setOnUpDelayed($("stream-quality-slider"), 1000, (value) => __sendParam("quality", value));

		tools.slider.setParams($("stream-h264-bitrate-slider"), 25, 20000, 25, 5000, function(value) {
			$("stream-h264-bitrate-value").innerText = value;
		});
		tools.slider.setOnUpDelayed($("stream-h264-bitrate-slider"), 1000, (value) => __sendParam("h264_bitrate", value));

		tools.slider.setParams($("stream-h264-gop-slider"), 0, 60, 1, 30, function(value) {
			$("stream-h264-gop-value").innerText = value;
		});
		tools.slider.setOnUpDelayed($("stream-h264-gop-slider"), 1000, (value) => __sendParam("h264_gop", value));

		tools.slider.setParams($("stream-desired-fps-slider"), 0, 120, 1, 0, function(value) {
			$("stream-desired-fps-value").innerText = (value === 0 ? "Unlimited" : value);
		});
		tools.slider.setOnUpDelayed($("stream-desired-fps-slider"), 1000, (value) => __sendParam("desired_fps", value));

		$("stream-resolution-selector").onchange = (() => __sendParam("resolution", $("stream-resolution-selector").value));

		tools.radio.setOnClick("stream-mode-radio", __clickModeRadio, false);

		// Not getInt() because of radio is a string container.
		// Also don't reset Streamer at class init.
		tools.radio.clickValue("stream-orient-radio", tools.storage.get("stream.orient", 0));
		tools.radio.setOnClick("stream-orient-radio", function() {
			if (["janus", "media"].includes(__streamer.getMode())) {
				let orient = parseInt(tools.radio.getValue("stream-orient-radio"));
				tools.storage.setInt("stream.orient", orient);
				if (__streamer.getOrientation() !== orient) {
					__resetStream();
				}
			}
		}, false);

		tools.slider.setParams($("stream-audio-volume-slider"), 0, 100, 1, 0, function(value) {
			$("stream-video").muted = !value;
			$("stream-video").volume = value / 100;
			$("stream-audio-volume-value").innerText = value + "%";
			if (__streamer.getMode() === "janus") {
				let allow_audio = !$("stream-video").muted;
				if (__streamer.isAudioAllowed() !== allow_audio) {
					__resetStream();
				}
			}
			tools.el.setEnabled($("stream-mic-switch"), !!value);
		});

		tools.storage.bindSimpleSwitch($("stream-mic-switch"), "stream.mic", false, function(allow_mic) {
			if (__streamer.getMode() === "janus") {
				if (__streamer.isMicAllowed() !== allow_mic) {
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

	self.ensureDeps = function(callback) {
		JanusStreamer.ensure_janus(function(avail) {
			__janus_imported = avail;
			callback();
		});
	};

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

	self.setState = function(state) {
		if (state) {
			if (!__state) {
				__state = {};
			}
			if (state.features !== undefined) {
				__state.features = state.features;
				__state.limits = state.limits; // Following together with features
			}
			if (__state.features !== undefined && state.streamer !== undefined) {
				__state.streamer = state.streamer;
				__setControlsEnabled(!!state.streamer);
			}
		} else {
			__state = null;
			__setControlsEnabled(false);
		}
		let visible = wm.isWindowVisible($("stream-window"));
		__applyState((visible && __state && __state.features) ? state : null);
	};

	var __applyState = function(state) {
		if (__janus_imported === null) {
			alert("__janus_imported is null, please report");
			return;
		}

		if (!state) {
			__streamer.stopStream();
			return;
		}

		if (state.features) {
			let f = state.features;
			let l = state.limits;
			let sup_h264 = $("stream-video").canPlayType("video/mp4; codecs=\"avc1.42E01F\"");
			let sup_vd = MediaStreamer.is_videodecoder_available();
			let sup_webrtc = JanusStreamer.is_webrtc_available();
			let has_media = (f.h264 && sup_vd); // Don't check sup_h264 for sure
			let has_janus = (__janus_imported && f.h264 && sup_webrtc); // Same

			tools.info(
				`Stream: Janus WebRTC state: features.h264=${f.h264},`
				+ ` webrtc=${sup_webrtc}, h264=${sup_h264}, janus_imported=${__janus_imported}`
			);

			tools.hidden.setVisible($("stream-message-no-webrtc"), __janus_imported && f.h264 && !sup_webrtc);
			tools.hidden.setVisible($("stream-message-no-vd"), f.h264 && !sup_vd);
			tools.hidden.setVisible($("stream-message-no-h264"), __janus_imported && f.h264 && !sup_h264);

			tools.slider.setRange($("stream-desired-fps-slider"), l.desired_fps.min, l.desired_fps.max);
			if (f.resolution) {
				let el = $("stream-resolution-selector");
				el.options.length = 0;
				for (let res of l.available_resolutions) {
					tools.selector.addOption(el, res, res);
				}
			} else {
				$("stream-resolution-selector").options.length = 0;
			}
			if (f.h264) {
				tools.slider.setRange($("stream-h264-bitrate-slider"), l.h264_bitrate.min, l.h264_bitrate.max);
				tools.slider.setRange($("stream-h264-gop-slider"), l.h264_gop.min, l.h264_gop.max);
			}

			// tools.feature.setEnabled($("stream-quality"), f.quality); // Only on s.encoder.quality
			tools.feature.setEnabled($("stream-resolution"), f.resolution);
			tools.feature.setEnabled($("stream-h264-bitrate"), f.h264);
			tools.feature.setEnabled($("stream-h264-gop"), f.h264);
			tools.feature.setEnabled($("stream-mode"), f.h264);
			if (!f.h264) {
				tools.feature.setEnabled($("stream-audio"), false);
				tools.feature.setEnabled($("stream-mic"), false);
			}

			let mode = tools.storage.get("stream.mode", "janus");
			if (mode === "janus" && !has_janus) {
				mode = "media";
			}
			if (mode === "media" && !has_media) {
				mode = "mjpeg";
			}
			tools.radio.clickValue("stream-mode-radio", mode);
		}

		if (state.streamer) {
			let s = state.streamer;
			__res = s.source.resolution;

			{
				let res = `${__res.width}x${__res.height}`;
				let el = $("stream-resolution-selector");
				if (!tools.selector.hasValue(el, res)) {
					tools.selector.addOption(el, res, res);
				}
				el.value = res;
			}
			tools.slider.setValue($("stream-quality-slider"), Math.max(s.encoder.quality, 1));
			tools.slider.setValue($("stream-desired-fps-slider"), s.source.desired_fps);
			if (s.h264 && s.h264.bitrate) {
				tools.slider.setValue($("stream-h264-bitrate-slider"), s.h264.bitrate);
				tools.slider.setValue($("stream-h264-gop-slider"), s.h264.gop); // Following together with gop
			}

			tools.feature.setEnabled($("stream-quality"), (s.encoder.quality > 0));

			__streamer.ensureStream(s);
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

	var __setControlsEnabled = function(enabled) {
		tools.el.setEnabled($("stream-quality-slider"), enabled);
		tools.el.setEnabled($("stream-desired-fps-slider"), enabled);
		tools.el.setEnabled($("stream-resolution-selector"), enabled);
		tools.el.setEnabled($("stream-h264-bitrate-slider"), enabled);
		tools.el.setEnabled($("stream-h264-gop-slider"), enabled);
	};

	var __setInfo = function(is_active, online, text) {
		$("stream-box").classList.toggle("stream-box-offline", !online);
		let el_grab = document.querySelector("#stream-window-header .window-grab");
		let el_info = $("stream-info");
		let title = `${__streamer.getName()} - `;
		if (is_active) {
			if (!online) {
				title += "No signal / ";
			}
			title += `${__res.width}x${__res.height}`;
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
		el_grab.innerText = el_info.innerText = title;
	};

	var __resetStream = function(mode=null) {
		if (mode === null) {
			mode = __streamer.getMode();
		}
		__streamer.stopStream();
		let orient = tools.storage.getInt("stream.orient", 0);
		if (mode === "janus") {
			let allow_audio = !$("stream-video").muted;
			let allow_mic = $("stream-mic-switch").checked;
			__streamer = new JanusStreamer(__setActive, __setInactive, __setInfo, orient, allow_audio, allow_mic);
			// Firefox doesn't support RTP orientation:
			//  - https://bugzilla.mozilla.org/show_bug.cgi?id=1316448
			tools.feature.setEnabled($("stream-orient"), !tools.browser.is_firefox);
		} else {
			if (mode === "media") {
				__streamer = new MediaStreamer(__setActive, __setInactive, __setInfo, orient);
				tools.feature.setEnabled($("stream-orient"), true);
			} else { // mjpeg
				__streamer = new MjpegStreamer(__setActive, __setInactive, __setInfo);
				tools.feature.setEnabled($("stream-orient"), false);
			}
			tools.feature.setEnabled($("stream-audio"), false); // Enabling in stream_janus.js
			tools.feature.setEnabled($("stream-mic"), false); // Ditto
		}
		if (wm.isWindowVisible($("stream-window"))) {
			__streamer.ensureStream((__state && __state.streamer !== undefined) ? __state.streamer : null);
		}
	};

	var __clickModeRadio = function() {
		let mode = tools.radio.getValue("stream-mode-radio");
		tools.storage.set("stream.mode", mode);
		if (mode !== __streamer.getMode()) {
			tools.hidden.setVisible($("stream-canvas"), (mode === "media"));
			tools.hidden.setVisible($("stream-image"), (mode === "mjpeg"));
			tools.hidden.setVisible($("stream-video"), (mode === "janus"));
			__resetStream(mode);
		}
	};

	var __clickScreenshotButton = function() {
		tools.windowOpen("api/streamer/snapshot");
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset stream?").then(function(ok) {
			if (ok) {
				__resetStream();
				tools.httpPost("api/streamer/reset", null, function(http) {
					if (http.status !== 200) {
						wm.error("Can't reset stream", http.responseText);
					}
				});
			}
		});
	};

	var __sendParam = function(name, value) {
		tools.httpPost("api/streamer/set_params", {[name]: value}, function(http) {
			if (http.status !== 200) {
				wm.error("Can't configure stream", http.responseText);
			}
		});
	};

	__init__();
}
