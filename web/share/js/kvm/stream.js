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


export function Streamer() {
	var self = this;

	/************************************************************************/

	var __resolution = {width: 640, height: 480};
	var __resolution_str = "640x480";

	var __size_factor = 1;

	var __client_key = tools.makeId();
	var __client_id = "";
	var __client_fps = -1;

	var __state_for_invisible = null;

	var __init__ = function() {
		$("stream-led").title = "Stream inactive";

		tools.sliderSetParams($("stream-quality-slider"), 5, 100, 5, 80);
		tools.sliderSetOnUp($("stream-quality-slider"), 1000, __updateQualityValue, (value) => __sendParam("quality", value));

		tools.sliderSetParams($("stream-desired-fps-slider"), 0, 120, 1, 0);
		tools.sliderSetOnUp($("stream-desired-fps-slider"), 1000, __updateDesiredFpsValue, (value) => __sendParam("desired_fps", value));

		$("stream-resolution-selector").onchange = (() => __sendParam("resolution", $("stream-resolution-selector").value));

		tools.sliderSetParams($("stream-size-slider"), 20, 200, 5, 100);
		$("stream-size-slider").oninput = () => __resize();
		$("stream-size-slider").onchange = () => __resize();

		tools.setOnClick($("stream-screenshot-button"), __clickScreenshotButton);
		tools.setOnClick($("stream-reset-button"), __clickResetButton);

		$("stream-window").show_hook = function() {
			if (__state_for_invisible !== null) {
				self.setState(__state_for_invisible);
			}
		};
	};

	/************************************************************************/

	self.setState = function(state) {
		if (!wm.isWindowVisible($("stream-window"))) {
			if (__state_for_invisible === null) {
				$("stream-image").src = "/share/png/blank-stream.png";
				$("stream-image").className = "stream-image-inactive";
				$("stream-box").classList.add("stream-box-inactive");
			}
			__state_for_invisible = state;
			state = null;
		} else {
			__state_for_invisible = null;
		}

		if (state) {
			tools.featureSetEnabled($("stream-quality"), state.features.quality && (state.streamer === null || state.streamer.encoder.quality > 0));
			tools.featureSetEnabled($("stream-resolution"), state.features.resolution);
		}

		if (state && state.streamer) {
			if (!$("stream-quality-slider").activated) {
				wm.switchEnabled($("stream-quality-slider"), true);
				if ($("stream-quality-slider").value !== state.streamer.encoder.quality) {
					$("stream-quality-slider").value = state.streamer.encoder.quality;
					__updateQualityValue(state.streamer.encoder.quality);
				}
			}

			if (!$("stream-desired-fps-slider").activated) {
				$("stream-desired-fps-slider").max = state.limits.max_fps;
				wm.switchEnabled($("stream-desired-fps-slider"), true);
				if ($("stream-desired-fps-slider").value !== state.streamer.source.desired_fps) {
					$("stream-desired-fps-slider").value = state.streamer.source.desired_fps;
					__updateDesiredFpsValue(state.streamer.source.desired_fps);
				}
			}

			if (
				__resolution.width !== state.streamer.source.resolution.width
				|| __resolution.height !== state.streamer.source.resolution.height
			) {
				__resolution = state.streamer.source.resolution;
				__resolution_str = `${__resolution.width}x${__resolution.height}`;
				if ($("stream-auto-resize-checkbox").checked) {
					__adjustSizeFactor();
				} else {
					__applySizeFactor();
				}
			}

			if (state.features.resolution) {
				if ($("stream-resolution-selector").resolutions !== state.limits.available_resolutions) {
					let resolutions_html = "";
					for (let variant of state.limits.available_resolutions) {
						resolutions_html += `<option value="${variant}">${variant}</option>`;
					}
					if (!state.limits.available_resolutions.includes(__resolution_str)) {
						resolutions_html += `<option value="${__resolution_str}">${__resolution_str}</option>`;
					}
					$("stream-resolution-selector").innerHTML = resolutions_html;
					$("stream-resolution-selector").resolutions = state.limits.available_resolutions;
				}
				document.querySelector(`#stream-resolution-selector [value="${__resolution_str}"]`).selected = true;
				wm.switchEnabled($("stream-resolution-selector"), true);
			}

			if (__ensureStream(state.streamer.stream.clients_stat)) {
				$("stream-led").className = "led-green";
				$("stream-led").title = "Stream is active";
				wm.switchEnabled($("stream-screenshot-button"), true);
				wm.switchEnabled($("stream-reset-button"), true);
				$("stream-quality-slider").activated = false;
				$("stream-desired-fps-slider").activated = false;

				tools.info("Stream: active");
			}

			__updateStreamWindow(true, state.streamer.source.online);

		} else {
			if ($("stream-led").className !== "led-gray") { // Чтобы не дублировать логи, когда окно стрима закрыто
				tools.info("Stream: inactive");
			}

			$("stream-led").className = "led-gray";
			$("stream-led").title = "Stream inactive";
			wm.switchEnabled($("stream-screenshot-button"), false);
			wm.switchEnabled($("stream-reset-button"), false);
			wm.switchEnabled($("stream-quality-slider"), false);
			wm.switchEnabled($("stream-desired-fps-slider"), false);
			wm.switchEnabled($("stream-resolution-selector"), false);

			__updateStreamWindow(false, false);
		}
	};

	var __ensureStream = function(clients_stat) {
		let stream_client = tools.getCookie("stream_client");
		if (!__client_id && stream_client && stream_client.startsWith(__client_key + "/")) {
			tools.info("Stream: found acceptable stream_client cookie:", stream_client);
			__client_id = stream_client.slice(stream_client.indexOf("/") + 1);
		}

		if (__client_id && __client_id in clients_stat) {
			__client_fps = clients_stat[__client_id].fps;
			return false;
		} else {
			__client_key = tools.makeId();
			__client_id = "";
			__client_fps = -1;

			let path = `/streamer/stream?key=${__client_key}`;
			if (tools.browser.is_safari || tools.browser.is_ios) {
				// uStreamer fix for WebKit
				tools.info("Stream: using dual_final_frames=1 to fix WebKit MJPG bugs");
				path += "&dual_final_frames=1";
			} else if (tools.browser.is_chrome || tools.browser.is_blink) {
				// uStreamer fix for Blink https://bugs.chromium.org/p/chromium/issues/detail?id=527446
				tools.info("Stream: using advance_headers=1 to fix Blink MJPG bugs");
				path += "&advance_headers=1";
			}

			tools.info("Stream: refreshing ...");
			$("stream-image").src = path;
			return true;
		}
	};

	var __updateStreamWindow = function(is_active, online) {
		if (online) {
			$("stream-image").className = "stream-image-active";
			$("stream-box").classList.remove("stream-box-inactive");
		} else {
			$("stream-image").className = "stream-image-inactive";
			$("stream-box").classList.add("stream-box-inactive");
		}

		let el_grab = document.querySelector("#stream-window-header .window-grab");
		let el_info = $("stream-info");
		if (is_active) {
			let title = "Stream &ndash; ";
			if (!online) {
				title += "no signal / ";
			}
			title += __resolution_str;
			if (__client_fps >= 0) {
				title += ` / ${__client_fps} fps`;
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

	var __resize = function() {
		let size = $("stream-size-slider").value;
		$("stream-size-value").innerHTML = `${size}%`;
		__size_factor = size / 100;
		__applySizeFactor();
	};

	var __adjustSizeFactor = function() {
		let el_window = $("stream-window");
		let el_slider = $("stream-size-slider");
		let view = wm.getViewGeometry();

		for (let size = 100; size >= el_slider.min; size -= el_slider.step) {
			tools.info("Stream: adjusting size:", size);
			$("stream-size-slider").value = size;
			__resize();

			let rect = el_window.getBoundingClientRect();
			if (
				rect.bottom <= view.bottom
				&& rect.top >= view.top
				&& rect.left >= view.left
				&& rect.right <= view.right
			) {
				break;
			}
		}
	};

	var __applySizeFactor = function() {
		let el = $("stream-image");
		el.style.width = __resolution.width * __size_factor + "px";
		el.style.height = __resolution.height * __size_factor + "px";
		wm.showWindow($("stream-window"), false);
	};

	__init__();
}
