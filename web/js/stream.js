function Stream() {
	// var self = this;

	/********************************************************************************/

	var __prev_state = false;
	var __resolution = {width: 640, height: 480};
	var __client_id = "";
	var __fps = 0;

	var __quality = 10;
	var __size_factor = 1;

	var __init__ = function() {
		$("stream-led").title = "Stream inactive";

		var quality = 10;
		$("stream-quality-select").innerHTML = "";
		for (; quality <= 100; quality += 10) {
			$("stream-quality-select").innerHTML += "<option value=\"" + quality + "\">" + quality + "%</option>";
		}

		tools.setOnClick($("stream-screenshot-button"), __clickScreenshotButton);

		$("stream-quality-select").onchange = __changeQuality;

		$("stream-size-slider").min = 50;
		$("stream-size-slider").max = 150;
		$("stream-size-slider").step = 10;
		$("stream-size-slider").value = 100;
		$("stream-size-slider").oninput = () => __resize;
		$("stream-size-slider").onchange = () => __resize;

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
					tools.info("Refreshing stream ...");
					$("stream-image").className = "stream-image-inactive";
					$("stream-box").classList.add("stream-box-inactive");
					$("stream-led").className = "led-off";
					$("stream-led").title = "Stream inactive";
					$("stream-screenshot-button").disabled = true;
					$("stream-quality-select").disabled = true;
					$("stream-reset-button").disabled = true;
					__updateStreamHeader(false);
					__fps = 0;
					__prev_state = false;

				} else if (http.status === 200) {
					if (__quality != response.source.quality) {
						document.querySelector("#stream-quality-select [value=\"" + response.source.quality + "\"]").selected = true;
						__quality = response.source.quality;
					}

					if (__resolution.width !== response.source.resolution.width || __resolution.height !== response.source.resolution.height) {
						__resolution = response.source.resolution;
						if ($("stream-auto-resize-checkbox").checked) {
							__adjustSizeFactor();
						} else {
							__applySizeFactor();
						}
					}

					var client_id = tools.getCookie("stream_client_id");
					if (client_id) {
						__client_id = client_id;
					}

					if (response.stream.clients_stat.hasOwnProperty(__client_id)) {
						__fps = response.stream.clients_stat[__client_id].fps;
					} else {
						__fps = 0;
					}

					__updateStreamHeader(true);

					if (!__prev_state) {
						tools.info("Stream acquired");
						$("stream-image").src = "/streamer/stream?t=" + new Date().getTime();
						$("stream-image").className = "stream-image-active";
						$("stream-box").classList.remove("stream-box-inactive");
						$("stream-led").className = "led-on";
						$("stream-led").title = "Stream is active";
						$("stream-screenshot-button").disabled = false;
						$("stream-quality-select").disabled = false;
						$("stream-reset-button").disabled = false;
						__prev_state = true;
					}
				}
			}
		});
		setTimeout(__startPoller, 1000);
	};

	var __updateStreamHeader = function(online) {
		var el_grab = document.querySelector("#stream-window-header .window-grab");
		if (online) {
			el_grab.innerHTML = "Stream &ndash; " + __resolution.width + "x" + __resolution.height + " / " + __fps + " fps";
		} else {
			el_grab.innerHTML = "Stream &ndash; offline";
		}
	};

	var __clickScreenshotButton = function() {
		var el_a = document.createElement("a");
		el_a.href = "/streamer/snapshot";
		el_a.download = "screenshot.jpg";
		document.body.appendChild(el_a);
		el_a.click();
		setTimeout(() => document.body.removeChild(el_a), 0);
	};

	var __clickResetButton = function() {
		$("stream-reset-button").disabled = true;
		var http = tools.makeRequest("POST", "/kvmd/streamer/reset", function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					ui.error("Can't reset stream:<br>", http.responseText);
				}
			}
		});
	};

	var __changeQuality = function() {
		var quality = parseInt($("stream-quality-select").value);
		if (__quality != quality) {
			$("stream-quality-select").disabled = true;
			var http = tools.makeRequest("POST", "/kvmd/streamer/set_params?quality=" + quality, function() {
				if (http.readyState === 4) {
					if (http.status !== 200) {
						ui.error("Can't configure stream:<br>", http.responseText);
					}
				}
			});
		}
	};

	var __resize = function(center=false) {
		var percent = $("stream-size-slider").value;
		$("stream-size-value").innerHTML = percent + "%";
		__size_factor = percent / 100;
		__applySizeFactor(center);
	};

	var __adjustSizeFactor = function() {
		var el_window = $("stream-window");
		var el_slider = $("stream-size-slider");
		var view = ui.getViewGeometry();

		for (var percent = 100; percent >= el_slider.min; percent -= el_slider.step) {
			tools.info("Adjusting size:", percent);
			$("stream-size-slider").value = percent;
			__resize(true);

			var rect = el_window.getBoundingClientRect();
			if (
				rect.bottom <= view.bottom
				&& rect.top >= view.top
				&& rect.left >= view.left
				&& rect.right <= view.right
			) {
				return;
			}
		}

		$("stream-size-slider").value = 100;
		__resize();
	};

	var __applySizeFactor = function(center=false) {
		var el_stream_image = $("stream-image");
		el_stream_image.style.width = __resolution.width * __size_factor + "px";
		el_stream_image.style.height = __resolution.height * __size_factor + "px";
		ui.showWindow($("stream-window"), false, center);
	};

	__init__();
}
