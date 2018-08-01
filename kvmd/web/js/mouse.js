var mouse = new function() {
	var __ws = null;
	var __current_pos = {x: 0, y:0};
	var __sent_pos = {x: 0, y:0};
	var __stream_hovered = false;

	this.init = function() {
		el_stream_box = $("stream-box");
		el_stream_box.onmouseenter = __hoverStream;
		el_stream_box.onmouseleave = __leaveStream;
		el_stream_box.onmousedown = (event) => __buttonHandler(event, true);
		el_stream_box.onmouseup = (event) => __buttonHandler(event, false);
		el_stream_box.oncontextmenu = (event) => event.preventDefault();
		el_stream_box.onmousemove = __moveHandler;
		el_stream_box.onwheel = (event) => __wheelHandler(event);
		setInterval(__sendMove, 100);
	};

	this.setSocket = function(ws) {
		__ws = ws;
		if (ws) {
			$("stream-box").classList.add("stream-box-mouse-enabled");
		} else {
			$("stream-box").classList.remove("stream-box-mouse-enabled");
		}
	};

	var __hoverStream = function() {
		__stream_hovered = true;
		mouse.updateLeds();
	};

	var __leaveStream = function() {
		__stream_hovered = false;
		mouse.updateLeds();
	};

	this.updateLeds = function() {
		$("hid-mouse-led").className = (__ws && __stream_hovered ? "led-on" : "led-off");
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
				el_stream_image = $("stream-image");
				__ws.send(JSON.stringify({
					event_type: "mouse_move",
					to: {
						x: __translate(pos.x, 0, el_stream_image.clientWidth, -32768, 32767),
						y: __translate(pos.y, 0, el_stream_image.clientHeight, -32768, 32767),
					},
				}));
			}
			__sent_pos = pos;
		}
	};

	var __translate = function(x, a, b, c, d) {
		return Math.round((x - a) / (b - a) * (d - c) + c);
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
