function Stream(ui) {
	// var self = this;

	/********************************************************************************/

	var __prev_state = false;
	var __normal_size = {width: 640, height: 480};
	var __size_factor = 1;

	var __init__ = function() {
		$("stream-led").title = "Stream inactive";

		$("stream-reset-button").onclick = __clickResetButton;
		$("stream-size-slider").oninput = __resize;
		$("stream-size-slider").onchange = __resize;

		__startPoller();
	};

	/********************************************************************************/

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
		setTimeout(__startPoller, 2000);
	};

	var __clickResetButton = function() {
		$("stream-reset-button").disabled = true;
		var http = tools.makeRequest("POST", "/kvmd/streamer/reset", function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					alert("Can't reset stream:", http.responseText);
				}
			}
		});
	};

	var __resize = function() {
		var percent = $("stream-size-slider").value;
		$("stream-size-counter").innerHTML = percent + "%";
		__size_factor = percent / 100;
		__applySizeFactor();
	};

	var __applySizeFactor = function() {
		var el_stream_image = $("stream-image");
		el_stream_image.style.width = __normal_size.width * __size_factor + "px";
		el_stream_image.style.height = __normal_size.height * __size_factor + "px";
		ui.showWindow($("stream-window"));
	};

	var __refreshImage = function() {
		var http = tools.makeRequest("GET", "/kvmd/streamer", function() {
			if (http.readyState === 4 && http.status === 200) {
				__normal_size = JSON.parse(http.responseText).result.size;
				__applySizeFactor();
				$("stream-image").src = "/streamer/?action=stream&time=" + new Date().getTime();
			}
		});
	};

	__init__();
}
