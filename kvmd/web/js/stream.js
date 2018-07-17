var stream = new function() {
	var __prev_state = false;

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
					$("stream-led").className = "led-off";
				} else if (!__prev_state) {
					__refreshImage();
					__prev_state = true;
					$("stream-image").className = "stream-image-active";
					$("stream-led").className = "led-on";
				}
			}
		});
		setTimeout(stream.startPoller, 2000);
	};

//	this.setActive = function(active) {
//		$("stream-image").className = (active ? "stream-image-active" : "stream-image-inactive");
//	};

	this.clickResetButton = function(el_button) {
		el_button.disabled = true;
		var http = tools.makeRequest("POST", "/kvmd/streamer/reset", function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					alert("Can't reset stream:", http.responseText);
				}
				el_button.disabled = false;
			}
		});
	};

	var __refreshImage = function() {
		var http = tools.makeRequest("GET", "/kvmd/streamer", function() {
			if (http.readyState === 4 && http.status === 200) {
				size = JSON.parse(http.responseText).result.size;
				el_stream_image = $("stream-image");
				el_stream_image.style.width = size.width + "px";
				el_stream_image.style.height = size.height + "px";
				el_stream_image.src = "/streamer/?action=stream&time=" + new Date().getTime();
			}
		});
	};
};
