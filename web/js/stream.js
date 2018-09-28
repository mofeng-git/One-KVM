function Stream() {
	// var self = this;

	/********************************************************************************/

	var __prev_state = false;

	var __quality = 10;

	var __normal_size = {width: 640, height: 480};
	var __size_factor = 1;

	var __init__ = function() {
		$("stream-led").title = "Stream inactive";

		var quality = 10;
		$("stream-quality-select").innerHTML = "";
		for (; quality <= 100; quality += 10) {
			$("stream-quality-select").innerHTML += "<option value=\"" + quality + "\">" + quality + "%</option>";
		}

		tools.setOnClick($("stream-reset-button"), __clickResetButton);
		$("stream-quality-select").onchange = __changeQuality;
		$("stream-size-slider").oninput = __resize;
		$("stream-size-slider").onchange = __resize;

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
					__prev_state = false;
					$("stream-image").className = "stream-image-inactive";
					$("stream-box").classList.add("stream-box-inactive");
					$("stream-led").className = "led-off";
					$("stream-led").title = "Stream inactive";
					$("stream-reset-button").disabled = true;
					$("stream-quality-select").disabled = true;
				} else if (http.status === 200) {
					if (__prev_state) {
						if (__normal_size != response.stream.resolution) {
							__normal_size = response.stream.resolution;
							__applySizeFactor();
						}
					} else {
						__normal_size = response.stream.resolution;
						__refreshImage();
						__prev_state = true;
						$("stream-image").className = "stream-image-active";
						$("stream-box").classList.remove("stream-box-inactive");
						$("stream-led").className = "led-on";
						$("stream-led").title = "Stream is active";
						$("stream-reset-button").disabled = false;
						$("stream-quality-select").disabled = false;
					}
				}
			}
		});
		setTimeout(__startPoller, 1000);
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

	var __resize = function() {
		var percent = $("stream-size-slider").value;
		$("stream-size-value").innerHTML = percent + "%";
		__size_factor = percent / 100;
		__applySizeFactor();
	};

	var __applySizeFactor = function() {
		var el_stream_image = $("stream-image");
		el_stream_image.style.width = __normal_size.width * __size_factor + "px";
		el_stream_image.style.height = __normal_size.height * __size_factor + "px";
		ui.showWindow($("stream-window"), false);
	};

	var __refreshImage = function() {
		var http = tools.makeRequest("GET", "/kvmd/streamer", function() {
			if (http.readyState === 4 && http.status === 200) {
				var result = JSON.parse(http.responseText).result;

				if (__quality != result.quality) {
					tools.info("Quality changed:", result.quality);
					document.querySelector("#stream-quality-select [value=\"" + result.quality + "\"]").selected = true;
					__quality = result.quality;
				}

				__applySizeFactor();
				$("stream-image").src = "/streamer/stream?t=" + new Date().getTime();
			}
		});
	};

	__init__();
}
