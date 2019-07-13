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


import {tools, $} from "../tools.js";
import {wm} from "../wm.js";


export function Streamer() {
	var self = this;

	/************************************************************************/

	var __resolution = {width: 640, height: 480};
	var __size_factor = 1;
	var __client_key = tools.makeId();
	var __client_id = "";
	var __client_fps = -1;
	var __prev = false;

	var __init__ = function() {
		$("stream-led").title = "Stream inactive";

		$("stream-quality-slider").min = 5;
		$("stream-quality-slider").max = 100;
		$("stream-quality-slider").step = 5;
		$("stream-quality-slider").value = 80;
		tools.setOnUpSlider($("stream-quality-slider"), 1000, __updateQualityValue, (value) => __sendParam("quality", value));

		$("stream-desired-fps-slider").min = 0;
		$("stream-desired-fps-slider").max = 120;
		$("stream-desired-fps-slider").step = 1;
		$("stream-desired-fps-slider").value = 0;
		tools.setOnUpSlider($("stream-desired-fps-slider"), 1000, __updateDesiredFpsValue, (value) => __sendParam("desired_fps", value));

		$("stream-size-slider").min = 20;
		$("stream-size-slider").max = 200;
		$("stream-size-slider").step = 5;
		$("stream-size-slider").value = 100;
		$("stream-size-slider").oninput = () => __resize();
		$("stream-size-slider").onchange = () => __resize();

		tools.setOnClick($("stream-screenshot-button"), __clickScreenshotButton);
		tools.setOnClick($("stream-reset-button"), __clickResetButton);
	};

	/************************************************************************/

	self.setState = function(state) {
		if (state && state.state) {
			let encoder = state.state.encoder;
			let source = state.state.source;
			let stream = state.state.stream;

			if (!__prev) {
				$("stream-quality-slider").activated = false;
				$("stream-desired-fps-slider").activated = false;
			}

			if (!$("stream-quality-slider").activated) {
				wm.switchDisabled($("stream-quality-slider"), false);
				if ($("stream-quality-slider").value !== encoder.quality) {
					$("stream-quality-slider").value = encoder.quality;
					__updateQualityValue(encoder.quality);
				}
			}

			if (!$("stream-desired-fps-slider").activated) {
				$("stream-desired-fps-slider").max = state.limits.max_fps;
				wm.switchDisabled($("stream-desired-fps-slider"), false);
				if ($("stream-desired-fps-slider").value !== source.desired_fps) {
					$("stream-desired-fps-slider").value = source.desired_fps;
					__updateDesiredFpsValue(source.desired_fps);
				}
			}

			if (__resolution.width !== source.resolution.width || __resolution.height !== source.resolution.height) {
				__resolution = source.resolution;
				if ($("stream-auto-resize-checkbox").checked) {
					__adjustSizeFactor();
				} else {
					__applySizeFactor();
				}
			}

			let stream_client = tools.getCookie("stream_client");
			if (!__client_id && stream_client && stream_client.startsWith(__client_key + "/")) {
				tools.info("Stream: found acceptable stream_client cookie:", stream_client);
				__client_id = stream_client.slice(stream_client.indexOf("/") + 1);
			}

			if (Object.prototype.hasOwnProperty.call(stream.clients_stat, __client_id)) {
				__client_fps = stream.clients_stat[__client_id].fps;
			} else {
				__clearState();
			}

			if (!__prev) {
				let path = "/streamer/stream?key=" + __client_key;
				if (tools.browser.is_safari || tools.browser.is_ios) {
					// uStreamer fix for WebKit
					tools.info("Stream: using dual_final_frames=1 to fix WebKit MJPG bugs");
					path += "&dual_final_frames=1";
				} else if (tools.browser.is_chrome || tools.browser.is_blink) {
					// uStreamer fix for Blink https://bugs.chromium.org/p/chromium/issues/detail?id=527446
					tools.info("Stream: using advance_headers=1 to fix Blink MJPG bugs");
					path += "&advance_headers=1";
				}
				$("stream-image").src = path;
				$("stream-image").className = "stream-image-active";
				$("stream-box").classList.remove("stream-box-inactive");
				$("stream-led").className = "led-green";
				$("stream-led").title = "Stream is active";
				wm.switchDisabled($("stream-screenshot-button"), false);
				wm.switchDisabled($("stream-reset-button"), false);
				tools.info("Stream: acquired");
				__prev = true;
			}

			__updateStreamHeader(true);

		} else {
			__clearState();
		}
	};

	var __clearState = function() {
		tools.info("Stream: refreshing ...");

		$("stream-image").className = "stream-image-inactive";
		$("stream-box").classList.add("stream-box-inactive");
		$("stream-led").className = "led-gray";
		$("stream-led").title = "Stream inactive";
		wm.switchDisabled($("stream-screenshot-button"), true);
		wm.switchDisabled($("stream-reset-button"), true);
		wm.switchDisabled($("stream-quality-slider"), true);
		wm.switchDisabled($("stream-desired-fps-slider"), true);

		__client_key = tools.makeId();
		__client_id = "";
		__client_fps = -1;
		__prev = false;
		__updateStreamHeader(false);
	};

	var __updateQualityValue = function(value) {
		$("stream-quality-value").innerHTML = value + "%";
	};

	var __updateDesiredFpsValue = function(value) {
		$("stream-desired-fps-value").innerHTML = (value === 0 ? "Unlimited" : value);
	};

	var __updateStreamHeader = function(online) {
		let el_grab = document.querySelector("#stream-window-header .window-grab");
		let el_info = $("stream-info");
		if (online) {
			let fps_suffix = (__client_fps >= 0 ? ` / ${__client_fps} fps` : "");
			el_grab.innerHTML = el_info.innerHTML = `Stream &ndash; ${__resolution.width}x${__resolution.height}${fps_suffix}`;
		} else {
			el_grab.innerHTML = el_info.innerHTML = "Stream &ndash; offline";
		}
	};

	var __clickScreenshotButton = function() {
		let el_a = document.createElement("a");
		el_a.href = "/streamer/snapshot";
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
		$("stream-size-value").innerHTML = size + "%";
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
		let el_stream_image = $("stream-image");
		el_stream_image.style.width = __resolution.width * __size_factor + "px";
		el_stream_image.style.height = __resolution.height * __size_factor + "px";
		wm.showWindow($("stream-window"), false);
	};

	__init__();
}
