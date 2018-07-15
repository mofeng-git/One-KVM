var mouse = new function() {
	var __send_move_timer = null;
	var __current_pos = {x: 0, y:0};
	var __sent_pos = {x: 0, y:0};

	this.installCapture = function(ws) {
		tools.info("Installing mouse capture ...");
		el_stream_image = $("stream-image");
		el_stream_image.onmousedown = (event) => __buttonHandler(ws, event, true);
		el_stream_image.onmouseup = (event) => __buttonHandler(ws, event, false);
		el_stream_image.oncontextmenu = (event) => event.preventDefault();
		el_stream_image.onmousemove = __moveHandler;
		el_stream_image.onwheel = (event) => __wheelHandler(ws, event);
		__send_move_timer = setInterval(() => __sendMove(ws), 100);
		$("hid-mouse-led").className = "led-on";
	};

	this.clearCapture = function() {
		tools.info("Removing mouse capture ...");
		if (__send_move_timer) {
			clearInterval(__send_move_timer);
			__send_move_timer = null;
		}
		__current_pos = {x: 0, y:0};
		__sent_pos = {x: 0, y:0};
		el_stream_image = $("stream-image");
		el_stream_image.onmousedown = null;
		el_stream_image.onmouseup = null;
		el_stream_image.oncontextmenu = null;
		el_stream_image.onmousemove = null;
		el_stream_image.onwheel = null;
		$("hid-mouse-led").className = "led-off";
	};

	var __buttonHandler = function(ws, event, state) {
		// https://www.w3schools.com/jsref/event_button.asp
		switch (event.button) {
			case 0: var button = "Left"; break;
			case 2: var button = "Right"; break;
			default: var button = null; break
		}
		if (button) {
			tools.debug("Mouse button", (state ? "pressed:" : "released:"), button);
			event.preventDefault();
			__sendMove(ws);
			ws.send(JSON.stringify({
				event_type: "mouse_button",
				button: button,
				state: state,
			}));
		}
	};

	var __moveHandler = function(event) {
		var rect = event.target.getBoundingClientRect();
		__current_pos = {
			x: Math.round(event.clientX - rect.left),
			y: Math.round(event.clientY - rect.top),
		};
	};

	var __sendMove = function(ws) {
		var pos = __current_pos;
		if (pos.x !== __sent_pos.x || pos.y !== __sent_pos.y) {
			tools.debug("Mouse move:", pos);
			ws.send(JSON.stringify({
				event_type: "mouse_move",
				to: pos,
			}));
			__sent_pos = pos;
		}
	};

	var __wheelHandler = function(ws, event) {
		// https://learn.javascript.ru/mousewheel
		if (event.preventDefault) {
			event.preventDefault();
		}
		delta = {x: event.deltaX, y: event.deltaY};
		tools.debug("Mouse wheel:", delta);
		ws.send(JSON.stringify({
			event_type: "mouse_wheel",
			delta: delta,
		}));
	};
};
