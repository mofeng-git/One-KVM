function Session(atx, hid, msd) {
	// var self = this;

	/********************************************************************************/

	var __ws = null;

	var __ping_timer = null;
	var __missed_heartbeats = 0;

	var __init__ = function() {
		$("link-led").title = "Not connected yet...";
		__loadKvmdVersion();
		__startPoller();
	};

	/********************************************************************************/

	var __loadKvmdVersion = function() {
		var http = tools.makeRequest("GET", "/kvmd/info", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					var version = JSON.parse(http.responseText).result.version;
					$("kvmd-version").innerHTML = "kvmd " + version.kvmd;
					$("about-version-kvmd").innerHTML = version.kvmd;
					$("about-version-python").innerHTML = version.python;
					$("about-version-platform").innerHTML = version.platform;
				} else {
					setTimeout(__loadKvmdVersion, 1000);
				}
			}
		});
	};

	var __startPoller = function() {
		$("link-led").className = "led-yellow";
		$("link-led").title = "Connecting...";
		var http = tools.makeRequest("GET", "/wsauth", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					__ws = new WebSocket((location.protocol === "https:" ? "wss" : "ws") + "://" + location.host + "/kvmd/ws");
					__ws.onopen = __wsOpenHandler;
					__ws.onmessage = __wsMessageHandler;
					__ws.onerror = __wsErrorHandler;
					__ws.onclose = __wsCloseHandler;
				} else {
					__wsCloseHandler(null);
				}
			}
		});
	};

	var __wsOpenHandler = function(event) {
		$("link-led").className = "led-green";
		$("link-led").title = "Connected";
		tools.debug("WebSocket opened:", event);
		atx.loadInitialState();
		msd.loadInitialState();
		hid.setSocket(__ws);
		__missed_heartbeats = 0;
		__ping_timer = setInterval(__pingServer, 1000);
	};

	var __wsMessageHandler = function(event) {
		// tools.debug("WebSocket: received data:", event.data);
		event = JSON.parse(event.data);
		if (event.msg_type === "pong") {
			__missed_heartbeats = 0;
		} else if (event.msg_type === "event") {
			if (event.msg.event === "atx_state") {
				atx.setState(event.msg.event_attrs);
			} else if (event.msg.event === "msd_state") {
				msd.setState(event.msg.event_attrs);
			}
		}
	};

	var __wsErrorHandler = function(event) {
		tools.error("WebSocket error:", event);
		if (__ws) {
			__ws.onclose = null;
			__ws.close();
			__wsCloseHandler(null);
		}
	};

	var __wsCloseHandler = function(event) {
		$("link-led").className = "led-gray";
		tools.debug("WebSocket closed:", event);
		if (__ping_timer) {
			clearInterval(__ping_timer);
			__ping_timer = null;
		}
		hid.setSocket(null);
		atx.clearState();
		__ws = null;
		setTimeout(function() {
			$("link-led").className = "led-yellow";
			setTimeout(__startPoller, 500);
		}, 500);
	};

	var __pingServer = function() {
		try {
			__missed_heartbeats += 1;
			if (__missed_heartbeats >= 5) {
				throw new Error("Too many missed heartbeats");
			}
			__ws.send(JSON.stringify({"event_type": "ping"}));
		} catch (err) {
			tools.error("Ping error:", err.message);
			if (__ws) {
				__ws.onclose = null;
				__ws.close();
				__wsCloseHandler(null);
			}
		}
	};

	__init__();
}
