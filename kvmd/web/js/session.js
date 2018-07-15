var session = new function() {
	var __ws = null;
	var __ping_timer = null;
	var __missed_heartbeats = 0;

	this.startPoller = function() {
		__ws = new WebSocket("ws://" + location.host + "/kvmd/ws");
		__ws.onopen = __wsOpenHandler;
		__ws.onmessage = __wsMessageHandler;
		__ws.onerror = __wsErrorHandler;
		__ws.onclose = __wsCloseHandler;
	};

	var __wsOpenHandler = function(event) {
		tools.debug("WebSocket opened:", event);
		hid.installCapture(__ws);
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
				atx.setLedsState(event.msg.event_attrs.leds);
			}
		}
	};

	var __wsErrorHandler = function(event) {
		tools.error("WebSocket error:", event);
		__ws.close();
		__ws = null;
	};

	var __wsCloseHandler = function(event) {
		tools.debug("WebSocket closed:", event);
		if (__ping_timer) {
			clearInterval(__ping_timer);
			__ping_timer = null;
		}
		hid.clearCapture();
		atx.clearLeds();
		setTimeout(session.startPoller, 1000);
	};

	var __pingServer = function(event) {
		try {
			__missed_heartbeats += 1;
			if (__missed_heartbeats >= 5) {
				throw new Error("Too many missed heartbeats");
			}
			__ws.send(JSON.stringify({"event_type": "ping"}));
		} catch (err) {
			tools.error("Ping error:", err.message);
			if (__ws) {
				__ws.close();
				__ws = null;
			}
		}
	};
};
