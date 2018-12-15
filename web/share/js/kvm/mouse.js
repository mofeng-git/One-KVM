function Mouse() {
	var self = this;

	/********************************************************************************/

	var __ws = null;

	var __current_pos = {x: 0, y:0};
	var __sent_pos = {x: 0, y:0};
	var __wheel_delta = {x: 0, y: 0};

	var __stream_hovered = false;

	var __init__ = function() {
		$("hid-mouse-led").title = "Mouse free";

		$("stream-box").onmouseenter = __hoverStream;
		$("stream-box").onmouseleave = __leaveStream;
		$("stream-box").onmousedown = (event) => __buttonHandler(event, true);
		$("stream-box").onmouseup = (event) => __buttonHandler(event, false);
		$("stream-box").oncontextmenu = (event) => event.preventDefault();
		$("stream-box").onmousemove = __moveHandler;
		$("stream-box").onwheel = __wheelHandler;
		$("stream-box").ontouchstart = (event) => __touchMoveHandler(event);

		Array.prototype.forEach.call(document.querySelectorAll("[data-mouse-button]"), function(el_button) {
			var button = el_button.getAttribute("data-mouse-button");
			tools.setOnDown(el_button, () => __sendButton(button, true));
			tools.setOnUp(el_button, () => __sendButton(button, false));
		});

		setInterval(__sendMove, 100);
	};

	/********************************************************************************/

	self.setSocket = function(ws) {
		__ws = ws;
		if (ws) {
			$("stream-box").classList.add("stream-box-mouse-enabled");
		} else {
			$("stream-box").classList.remove("stream-box-mouse-enabled");
		}
		__updateLeds();
	};

	var __hoverStream = function() {
		__stream_hovered = true;
		__updateLeds();
	};

	var __leaveStream = function() {
		__stream_hovered = false;
		__updateLeds();
	};

	var __updateLeds = function() {
		if (__ws && (__stream_hovered || tools.browser.is_ios)) {
			// Mouse is always available on iOS via touchscreen
			$("hid-mouse-led").className = "led-green";
			$("hid-mouse-led").title = "Mouse tracked";
		} else {
			$("hid-mouse-led").className = "led-gray";
			$("hid-mouse-led").title = "Mouse free";
		}
	};

	var __buttonHandler = function(event, state) {
		// https://www.w3schools.com/jsref/event_button.asp
		event.preventDefault();
		switch (event.button) {
			case 0: __sendButton("left", state); break;
			case 2: __sendButton("right", state); break;
		}
	};

	var __touchMoveHandler = function(event) {
		event.preventDefault();
		if (event.touches[0].target && event.touches[0].target.getBoundingClientRect) {
			var rect = event.touches[0].target.getBoundingClientRect();
			__current_pos = {
				x: Math.round(event.touches[0].clientX - rect.left),
				y: Math.round(event.touches[0].clientY - rect.top),
			};
			__sendMove();
		}
	};

	var __moveHandler = function(event) {
		var rect = event.target.getBoundingClientRect();
		__current_pos = {
			x: Math.round(event.clientX - rect.left),
			y: Math.round(event.clientY - rect.top),
		};
	};


	var __sendButton = function(button, state) {
		tools.debug("Mouse: button", (state ? "pressed:" : "released:"), button);
		__sendMove();
		if (__ws) {
			__ws.send(JSON.stringify({
				event_type: "mouse_button",
				button: button,
				state: state,
			}));
		}
	};

	var __sendMove = function() {
		var pos = __current_pos;
		if (pos.x !== __sent_pos.x || pos.y !== __sent_pos.y) {
			var el_stream_image = $("stream-image");
			var to = {
				x: __translate(pos.x, 0, el_stream_image.clientWidth, -32768, 32767),
				y: __translate(pos.y, 0, el_stream_image.clientHeight, -32768, 32767),
			};

			tools.debug("Mouse: moved:", to);
			if (__ws) {
				__ws.send(JSON.stringify({
					event_type: "mouse_move",
					to: to,
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

		var delta = {x: 0, y: 0};

		__wheel_delta.y += event.deltaY;
		if (Math.abs(__wheel_delta.y) >= 100) {
			delta.y = __wheel_delta.y / Math.abs(__wheel_delta.y) * (-5);
			__wheel_delta.y = 0;
		}

		if (delta.y) {
			tools.debug("Mouse: scrolled:", delta);
			if (__ws) {
				__ws.send(JSON.stringify({
					event_type: "mouse_wheel",
					delta: delta,
				}));
			}
		}
	};

	__init__();
}
