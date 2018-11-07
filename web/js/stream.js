function Stream() {
	// var self = this;

	/********************************************************************************/

	var __prev_state = false;
	var __resolution = {width: 640, height: 480};
	var __size_factor = 1;
	var __key = tools.makeId();
	var __client_id = "";
	var __fps = -1;

	var __init__ = function() {
		$("stream-led").title = "Stream inactive";

		$("stream-quality-slider").min = 10;
		$("stream-quality-slider").max = 100;
		$("stream-quality-slider").step = 5;
		$("stream-quality-slider").value = 80;
		tools.setOnUpSlider($("stream-quality-slider"), 1000, function(value) {
			$("stream-quality-value").innerHTML = value + "%";
		}, __sendQuality);

		$("stream-soft-fps-slider").min = 1;
		$("stream-soft-fps-slider").max = 30;
		$("stream-soft-fps-slider").step = 1;
		$("stream-soft-fps-slider").value = 30;
		tools.setOnUpSlider($("stream-soft-fps-slider"), 1000, function(value) {
			$("stream-soft-fps-value").innerHTML = value;
		}, __sendSoftFps);

		$("stream-size-slider").min = 20;
		$("stream-size-slider").max = 200;
		$("stream-size-slider").step = 5;
		$("stream-size-slider").value = 100;
		$("stream-size-slider").oninput = () => __resize();
		$("stream-size-slider").onchange = () => __resize();

		tools.setOnClick($("stream-screenshot-button"), __clickScreenshotButton);
		tools.setOnClick($("stream-reset-button"), __clickResetButton);

		__startPoller();
	};

	/********************************************************************************/

	// XXX: In current implementation we don't need this event because Stream() has own state poller

	var __startPoller = function() {
		var http = tools.makeRequest("GET", "/streamer/ping", function() {
			if (http.readyState === 4) {
				var response = (http.status === 200 ? JSON.parse(http.responseText) : null);

				if (http.status !== 200) {
					if (__prev_state) {
						tools.info("Stream: refreshing ...");
						$("stream-image").className = "stream-image-inactive";
						$("stream-box").classList.add("stream-box-inactive");
						$("stream-led").className = "led-gray";
						$("stream-led").title = "Stream inactive";
						$("stream-screenshot-button").disabled = true;
						__setStreamerControlsDisabled(true);
						__updateStreamHeader(false);
						__key = tools.makeId();
						__client_id = "";
						__fps = -1;
						__prev_state = false;
					}

				} else if (http.status === 200) {
					if (!$("stream-soft-fps-slider").activated) {
						$("stream-soft-fps-slider").disabled = false;
						if ($("stream-soft-fps-slider").value !== response.source.soft_fps) {
							$("stream-soft-fps-slider").value = response.source.soft_fps;
							$("stream-soft-fps-value").innerHTML = response.source.soft_fps;
						}
					}

					if (!$("stream-quality-slider").activated) {
						$("stream-quality-slider").disabled = false;
						if ($("stream-quality-slider").value !== response.source.quality) {
							$("stream-quality-slider").value = response.source.quality;
							$("stream-quality-value").innerHTML = response.source.quality + "%";
						}
					}

					if (__resolution.width !== response.source.resolution.width || __resolution.height !== response.source.resolution.height) {
						__resolution = response.source.resolution;
						if ($("stream-auto-resize-checkbox").checked) {
							__adjustSizeFactor();
						} else {
							__applySizeFactor();
						}
					}

					var stream_client = tools.getCookie("stream_client");
					if (!__client_id && stream_client && stream_client.startsWith(__key + "/")) {
						tools.info("Stream: found acceptable stream_client cookie:", stream_client);
						__client_id = stream_client.slice(stream_client.indexOf("/") + 1);
					}

					if (response.stream.clients_stat.hasOwnProperty(__client_id)) {
						__fps = response.stream.clients_stat[__client_id].fps;
					} else {
						__fps = -1;
					}

					__updateStreamHeader(true);

					if (!__prev_state) {
						var path = "/streamer/stream?key=" + __key;
						if (tools.browser.is_chrome || tools.browser.is_blink) {
							// uStreamer fix for Blink https://bugs.chromium.org/p/chromium/issues/detail?id=527446
							tools.info("Stream: using advance_headers=1 to fix Blink MJPG bugs");
							path += "&advance_headers=1";
						} else if (tools.browser.is_safari || tools.browser.is_ios) {
							// uStreamer fix for WebKit
							tools.info("Stream: using dual_final_frames=1 to fix WebKit MJPG bugs");
							path += "&dual_final_frames=1";
						}
						$("stream-image").src = path;
						$("stream-image").className = "stream-image-active";
						$("stream-box").classList.remove("stream-box-inactive");
						$("stream-led").className = "led-green";
						$("stream-led").title = "Stream is active";
						$("stream-screenshot-button").disabled = false;
						$("stream-reset-button").disabled = false;
						__prev_state = true;
						tools.info("Stream: acquired");
					}
				}
			}
		});
		setTimeout(__startPoller, 1000);
	};

	var __updateStreamHeader = function(online) {
		var el_grab = document.querySelector("#stream-window-header .window-grab");
		var el_info = $("stream-info");
		if (online) {
			var fps_suffix = (__fps >= 0 ? " / " + __fps + " fps" : "");
			el_grab.innerHTML = el_info.innerHTML = "Stream &ndash; " + __resolution.width + "x" + __resolution.height + fps_suffix;
		} else {
			el_grab.innerHTML = el_info.innerHTML = "Stream &ndash; offline";
		}
	};

	var __clickScreenshotButton = function() {
		var el_a = document.createElement("a");
		el_a.href = "/streamer/snapshot";
		el_a.target = "_blank";
		document.body.appendChild(el_a);
		el_a.click();
		setTimeout(() => document.body.removeChild(el_a), 0);
	};

	var __clickResetButton = function() {
		__setStreamerControlsDisabled(true);
		var http = tools.makeRequest("POST", "/kvmd/streamer/reset", function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					ui.error("Can't reset stream:<br>", http.responseText);
				}
			}
		});
	};

	var __sendQuality = function(value) {
		__setStreamerControlsDisabled(true);
		var http = tools.makeRequest("POST", "/kvmd/streamer/set_params?quality=" + value, function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					ui.error("Can't configure stream:<br>", http.responseText);
				}
				$("stream-quality-slider").activated = false;
			}
		});
	};

	var __sendSoftFps = function(value) {
		__setStreamerControlsDisabled(true);
		var http = tools.makeRequest("POST", "/kvmd/streamer/set_params?soft_fps=" + value, function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					ui.error("Can't configure stream:<br>", http.responseText);
				}
				$("stream-soft-fps-slider").activated = false;
			}
		});
	};

	var __setStreamerControlsDisabled = function(disabled) {
		$("stream-reset-button").disabled = disabled;
		$("stream-quality-slider").disabled = disabled;
		$("stream-soft-fps-slider").disabled = disabled;
	};

	var __resize = function(center=false) {
		var size = $("stream-size-slider").value;
		$("stream-size-value").innerHTML = size + "%";
		__size_factor = size / 100;
		__applySizeFactor(center);
	};

	var __adjustSizeFactor = function() {
		var el_window = $("stream-window");
		var el_slider = $("stream-size-slider");
		var view = ui.getViewGeometry();

		for (var size = 100; size >= el_slider.min; size -= el_slider.step) {
			tools.info("Stream: adjusting size:", size);
			$("stream-size-slider").value = size;
			__resize(true);

			var rect = el_window.getBoundingClientRect();
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

	var __applySizeFactor = function(center=false) {
		var el_stream_image = $("stream-image");
		el_stream_image.style.width = __resolution.width * __size_factor + "px";
		el_stream_image.style.height = __resolution.height * __size_factor + "px";
		ui.showWindow($("stream-window"), false, center);
	};

	__init__();
}
