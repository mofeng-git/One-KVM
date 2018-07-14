function runKvmdUi() {
	__startSessionPoller();
	__startStreamPoller();
}


// -----------------------------------------------------------------------------
function __startSessionPoller() {
	var ws = new WebSocket("ws://" + location.host + "/kvmd/ws");

	ws.onopen = function(event) {
		__installHidHandlers(ws);
		__setSessionStatus(true);
		__startSessionPoller.ping_server_timer = setInterval(() => __pingServer(ws), 2000);
	};

	ws.onmessage = function(event) {
		// console.log("KVMD:", event.data);
		event = JSON.parse(event.data);
		if (event.msg_type == "event") {
			if (event.msg.event == "pong") {
				__pingServer.missed_heartbeats = 0;
			} else if (event.msg.event == "atx_state") {
				leds = event.msg.event_attrs.leds;
				document.getElementById("atx-power-led").className = (leds.power ? "led-on" : "led-off");
				document.getElementById("atx-hdd-led").className = (leds.hdd ? "led-busy" : "led-off");
			}
		}
	};

	ws.onclose = function(event) {
		if (__startSessionPoller.ping_server_timer) {
			clearInterval(__startSessionPoller.ping_server_timer);
		}
		__clearHidHandlers();
		__setSessionStatus(false);
		document.getElementById("atx-power-led").className = "led-off";
		document.getElementById("atx-hdd-led").className = "led-off";
		setTimeout(__startSessionPoller, 2000);
	};

	ws.onerror = function(error) {
		ws.close();
	};
}

function __pingServer(ws) {
	try {
		__pingServer.missed_heartbeats++;
		if (__pingServer.missed_heartbeats >= 5) {
			throw new Error("Too many missed heartbeats");
		}
		ws.send(JSON.stringify({"event_type": "ping"}));
	} catch (err) {
		__pingServer.missed_heartbeats = 0;
		console.warn("Closing session:", err.message);
		ws.close();
	}
}
__pingServer.missed_heartbeats = 0;

function __setSessionStatus(status) {
	var el_session_status = document.getElementById("session-status");
	el_session_status.innerHTML = (status ? "Session active" : "Session closed, trying to reconnect...");
	el_session_status.className = (status ? "session-active" : "session-closed");
}

function __installHidHandlers(ws) {
	var http = __request("GET", "/kvmd/hid", function() {
		if (http.readyState == 4) {
			if (http.status == 200) {
				features = JSON.parse(http.responseText).result.features;
				if (features.keyboard) {
					// https://www.codeday.top/2017/05/03/24906.html
					document.onkeydown = (event) => __onKeyEvent(ws, event, true);
					document.onkeyup = (event) => __onKeyEvent(ws, event, false);
					document.getElementById("hid-keyboard-led").className = "led-on";
				}
				if (features.mouse) {
					el_stream_image = document.getElementById("stream-image");
					el_stream_image.onmousedown = (event) => __onMouseButton(ws, event, true);
					el_stream_image.onmouseup = (event) => __onMouseButton(ws, event, false);
					el_stream_image.oncontextmenu = (event) => event.preventDefault();
					el_stream_image.onmousemove = __onMouseMove;
					el_stream_image.onwheel = (event) => __onMouseWheel(ws, event);
					document.getElementById("hid-mouse-led").className = "led-on";

					__installHidHandlers.mouse_move_timer = setInterval(() => __handleMouseMove(ws), 100);
				}
			} else {
				alert("Can't fetch HID features:", http.responseText);
			}
		}
	});
}

function __clearHidHandlers() {
	if (__installHidHandlers.mouse_move_timer) {
		clearInterval(__installHidHandlers.mouse_move_timer);
	}

	document.onkeydown = null;
	document.onkeyup = null;
	document.getElementById("hid-keyboard-led").className = "led-off";

	el_stream_image = document.getElementById("stream-image");
	el_stream_image.onmousedown = null;
	el_stream_image.onmouseup = null;
	el_stream_image.oncontextmenu = null;
	el_stream_image.onmousemove = null;
	el_stream_image.onwheel = null;
	document.getElementById("hid-mouse-led").className = "led-off";
}

function __onKeyEvent(ws, event, state) {
	// console.log("KVMD: Key", (state ? "pressed:" : "released:"), event)
	if (!event.metaKey) { // https://github.com/wesbos/keycodes/blob/gh-pages/scripts.js
		event.preventDefault();
	}
	ws.send(JSON.stringify({
		event_type: "key",
		key: event.code,
		state: state,
	}));
}

function __onMouseButton(ws, event, state) {
	// https://www.w3schools.com/jsref/event_button.asp
	switch (event.button) {
		case 0: var button = "Left"; break;
		case 2: var button = "Right"; break;
		default: var button = null; break
	}
	if (button) {
		// console.log("KVMD: Mouse button", (state ? "pressed:" : "released:"), button);
		event.preventDefault();
		__handleMouseMove(ws);
		ws.send(JSON.stringify({
			event_type: "mouse_button",
			button: button,
			state: state,
		}));
	}
}

function __onMouseMove(event) {
	var rect = event.target.getBoundingClientRect();
	__onMouseMove.pos = {
		x: Math.round(event.clientX - rect.left),
		y: Math.round(event.clientY - rect.top),
	};
}
__onMouseMove.pos = {x: 0, y:0};

function __handleMouseMove(ws) {
	var pos = __onMouseMove.pos;
	if (pos.x != __handleMouseMove.old_pos.x || pos.y != __handleMouseMove.old_pos.y) {
		ws.send(JSON.stringify({
			event_type: "mouse_move",
			to: pos,
		}));
		__handleMouseMove.old_pos = pos;
	}
}
__handleMouseMove.old_pos = {x: 0, y:0};

function __onMouseWheel(ws, event) {
	// https://learn.javascript.ru/mousewheel
	if (event.preventDefault) {
		event.preventDefault();
	}
	ws.send(JSON.stringify({
		event_type: "mouse_wheel",
		delta: {
			x: event.deltaX,
			y: event.deltaY,
		},
	}));
}


// -----------------------------------------------------------------------------
function clickAtxButton(el_button) {
	switch (el_button.id) {
		case "atx-power-button":
			var button = "power";
			var confirm_msg = "Are you sure to click the power button?";
			break;
		case "atx-power-button-long":
			var button = "power_long";
			var confirm_msg = "Are you sure to perform the long press of the power button?";
			break;
		case "atx-reset-button":
			var button = "reset";
			var confirm_msg = "Are you sure to reboot the server?";
			break;
		default:
			var button = null;
			var confirm_msg = null;
			break;
	}

	if (button && confirm(confirm_msg)) {
		__setAtxButtonsBusy(true);
		var http = __request("POST", "/kvmd/atx/click?button=" + button, function() {
			if (http.readyState == 4) {
				if (http.status == 409) {
					alert("Performing another ATX operation for other client, please try again later");
				} else if (http.status != 200) {
					alert("Click error:", http.responseText);
				}
				__setAtxButtonsBusy(false);
			}
		});
	}
}

function __setAtxButtonsBusy(busy) {
	[
		"atx-power-button",
		"atx-power-button-long",
		"atx-reset-button",
	].forEach(function(name) {
		__setButtonBusy(document.getElementById(name), busy);
	});
}


// -----------------------------------------------------------------------------
function __startStreamPoller() {
	var http = __request("GET", "/streamer/?action=snapshot", function() {
		if (http.readyState == 2 || http.readyState == 4) {
			var status = http.status;
			http.onreadystatechange = null;
			http.abort();
			if (status != 200) {
				console.log("Refreshing stream ...");
				__startStreamPoller.last = false;
				document.getElementById("stream-image").className = "stream-image-off";
				document.getElementById("screen-led").className = "led-off";
			} else if (!__startStreamPoller.last) {
				__refreshStream();
				__startStreamPoller.last = true;
				document.getElementById("stream-image").className = "stream-image-on";
				document.getElementById("screen-led").className = "led-on";
			}
		}
	});
	setTimeout(__startStreamPoller, 2000);
}
__startStreamPoller.last = false;

function __refreshStream() {
	var http = __request("GET", "/kvmd/streamer", function() {
		if (http.readyState == 4 && http.status == 200) {
			size = JSON.parse(http.responseText).result.size;
			el_stream_box = document.getElementById("stream-image");
			el_stream_box.style.width = size.width + "px";
			el_stream_box.style.height = size.height + "px";
			document.getElementById("stream-image").src = "/streamer/?action=stream&time=" + new Date().getTime();
		}
	});
}

function clickResetStreamButton(el_button) {
	__setButtonBusy(el_button, true);
	var http = __request("POST", "/kvmd/streamer/reset", function() {
		if (http.readyState == 4) {
			if (http.status != 200) {
				alert("Can't reset stream:", http.responseText);
			}
			__setButtonBusy(el_button, false);
		}
	});
}


// -----------------------------------------------------------------------------
function __request(method, url, callback) {
	var http = new XMLHttpRequest();
	http.open(method, url, true)
	http.onreadystatechange = callback;
	http.send();
	return http;
}

function __setButtonBusy(el_button, busy) {
	el_button.disabled = busy;
	el_button.style.cursor = (busy ? "wait" : "default");
}
