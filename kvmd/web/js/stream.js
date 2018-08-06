var stream = new function() {
	var __prev_state = false;
	var __normal_size = {width: 640, height: 480};
	var __size_factor = 1;

	this.startPoller = function() {
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
					$("stream-reset-button").disabled = true;
				} else if (!__prev_state) {
					__refreshImage();
					__prev_state = true;
					$("stream-image").className = "stream-image-active";
					$("stream-box").classList.remove("stream-box-inactive");
					$("stream-led").className = "led-on";
					$("stream-reset-button").disabled = false;
				}
			}
		});
		setTimeout(stream.startPoller, 2000);
	};

	this.clickResetButton = function() {
		$("stream-reset-button").disabled = true;
		var http = tools.makeRequest("POST", "/kvmd/streamer/reset", function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					alert("Can't reset stream:", http.responseText);
				}
			}
		});
	};

	this.resize = function(percent) {
		$("stream-size-counter").innerHTML = percent + "%";
		__size_factor = percent / 100;
		__applySizeFactor();
	};

	var __applySizeFactor = function() {
		var el_stream_image = $("stream-image");
		el_stream_image.style.width = __normal_size.width * __size_factor + "px";
		el_stream_image.style.height = __normal_size.height * __size_factor + "px";
	};

	var __refreshImage = function() {
		var http = tools.makeRequest("GET", "/kvmd/streamer", function() {
			if (http.readyState === 4 && http.status === 200) {
				__normal_size = JSON.parse(http.responseText).result.size;
				__applySizeFactor();
				$("stream-image").src = "/streamer/?action=stream&time=" + new Date().getTime();
				ui.showWindow("stream-window");
			}
		});
	};
};
