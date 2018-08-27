function Stream(ui) {
	// var self = this;

	/********************************************************************************/

	var __prev_state = false;

	var __resolution = "640x480";
	var __resolutions = ["640x480"];

	var __normal_size = {width: 640, height: 480};
	var __size_factor = 1;

	var __init__ = function() {
		$("stream-led").title = "Stream inactive";

		$("stream-reset-button").onclick = __clickResetButton;
		$("stream-resolution-select").onchange = __changeResolution;
		$("stream-size-slider").oninput = __resize;
		$("stream-size-slider").onchange = __resize;

		__startPoller();
	};

	/********************************************************************************/

	// XXX: In current implementation we don't need this event because Stream() has own state poller

	var __startPoller = function() {
		var http = tools.makeRequest("GET", "/streamer/?action=snapshot", function() {
			if (http.readyState === 2 || http.readyState === 4) {
				var status = http.status;
				http.onreadystatechange = null;
				http.abort();
				if (status !== 200) {
					tools.info("Refreshing stream ...");
					__prev_state = false;
					$("stream-image").className = "stream-image-inactive";
					$("stream-box").classList.add("stream-box-inactive");
					$("stream-led").className = "led-off";
					$("stream-led").title = "Stream inactive";
					$("stream-reset-button").disabled = true;
					$("stream-resolution-select").disabled = true;
				} else if (!__prev_state) {
					__refreshImage();
					__prev_state = true;
					$("stream-image").className = "stream-image-active";
					$("stream-box").classList.remove("stream-box-inactive");
					$("stream-led").className = "led-on";
					$("stream-led").title = "Stream is active";
					$("stream-reset-button").disabled = false;
				}
			}
		});
		setTimeout(__startPoller, 1500);
	};

	var __clickResetButton = function() {
		$("stream-reset-button").disabled = true;
		var http = tools.makeRequest("POST", "/kvmd/streamer/reset", function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					modal.error("Can't reset stream:<br>", http.responseText);
				}
			}
		});
	};

	var __changeResolution = function() {
		var resolution = $("stream-resolution-select").value;
		if (__resolution != resolution) {
			$("stream-resolution-select").disabled = true;
			var http = tools.makeRequest("POST", "/kvmd/streamer/set_params?resolution=" + resolution, function() {
				if (http.readyState === 4) {
					if (http.status !== 200) {
						modal.error("Can't configure stream:<br>", http.responseText);
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

				if (__resolutions != result.resolutions) {
					tools.info("Resolutions list changed:", result.resolutions);
					$("stream-resolution-select").innerHTML = "";
					result.resolutions.forEach(function(resolution) {
						$("stream-resolution-select").innerHTML += "<option value=\"" + resolution + "\">" + resolution + "</option>";
					});
					$("stream-resolution-select").disabled = (result.resolutions.length == 1);
					__resolutions = result.resolutions;
				}

				if (__resolution != result.resolution) {
					tools.info("Resolution changed:", result.resolution);
					document.querySelector("#stream-resolution-select [value=\"" + result.resolution + "\"]").selected = true;
					__resolution = result.resolution;
				}

				__normal_size = result.size;
				__applySizeFactor();
				$("stream-image").src = "/streamer/?action=stream&time=" + new Date().getTime();
			}
		});
	};

	__init__();
}
