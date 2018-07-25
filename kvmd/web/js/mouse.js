var mouse = new function() {
	var __ws = null;
	var __current_pos = {x: 0, y:0};
	var __sent_pos = {x: 0, y:0};

	this.init = function() {
		el_stream_image = $("stream-image");
		el_stream_image.onmousedown = (event) => __buttonHandler(event, true);
		el_stream_image.onmouseup = (event) => __buttonHandler(event, false);
		el_stream_image.oncontextmenu = (event) => event.preventDefault();
		el_stream_image.onmousemove = __moveHandler;
		el_stream_image.onwheel = (event) => __wheelHandler(event);
		setInterval(__sendMove, 100);
	};

	this.setSocket = function(ws) {
		$("hid-mouse-led").className = (ws ? "led-on" : "led-off");
		__ws = ws;
	};

	var __buttonHandler = function(event, state) {
		// https://www.w3schools.com/jsref/event_button.asp
		switch (event.button) {
			case 0: var button = "left"; break;
			case 2: var button = "right"; break;
			default: var button = null; break;
		}
		if (button) {
			event.preventDefault();
			tools.debug("Mouse button", (state ? "pressed:" : "released:"), button);
			__sendMove();
			if (__ws) {
				__ws.send(JSON.stringify({
					event_type: "mouse_button",
					button: button,
					state: state,
				}));
			}
		}
	};

	var __moveHandler = function(event) {
		var rect = event.target.getBoundingClientRect();
		__current_pos = {
			x: Math.round(event.clientX - rect.left),
			y: Math.round(event.clientY - rect.top),
		};
	};

	var __sendMove = function() {
		var pos = __current_pos;
		if (pos.x !== __sent_pos.x || pos.y !== __sent_pos.y) {
			tools.debug("Mouse move:", pos);
			if (__ws) {
				__ws.send(JSON.stringify({
					event_type: "mouse_move",
					to: pos,
				}));
			}
			__sent_pos = pos;
		}
	};

	var __wheelHandler = function(event) {
		// https://learn.javascript.ru/mousewheel
		if (event.preventDefault) {
			event.preventDefault();
		}
		delta = {x: event.deltaX, y: event.deltaY};
		tools.debug("Mouse wheel:", delta);
		if (__ws) {
			__ws.send(JSON.stringify({
				event_type: "mouse_wheel",
				delta: delta,
			}));
		}
	};
};
