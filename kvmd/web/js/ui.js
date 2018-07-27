var ui = new function() {
	var __top_z_index = 1;

	this.init = function() {
		Array.prototype.forEach.call(document.getElementsByClassName("ctl-item"), function(el_item) {
			el_item.onclick = function() { __toggleMenu(el_item); };
		});

		window.onclick = __windowClickHandler;
		Array.prototype.forEach.call(document.getElementsByClassName("window"), function(el_window) {
			var el_grab = el_window.querySelector(".window-header .window-grab");
			__makeWindowMovable(el_grab, el_window);

			var el_button = el_window.querySelector(".window-header .window-button-close");
			if (el_button) {
				el_button.onclick = function() {
					el_window.style.display = "none";
				};
			}
		});

		if (typeof document.hidden !== "undefined") {
			__hidden_attr = "hidden";
			__visibility_change_attr = "visibilitychange";
		} else if (typeof document.webkitHidden !== "undefined") {
			__hidden_attr = "webkitHidden";
			__visibility_change_attr = "webkitvisibilitychange";
		} else if (typeof document.mozHidden !== "undefined") {
			__hidden_attr = "mozHidden";
			__visibility_change_attr = "mozvisibilitychange";
		}

		if (__visibility_change_attr) {
			document.addEventListener(
				__visibility_change_attr,
				function() {
					if (document[__hidden_attr]) {
						hid.releaseAll();
					}
				},
				false,
			);
		}

		window.onpagehide = hid.releaseAll;
		window.onblur = hid.releaseAll;
	};

	this.showWindow = function(id) {
		var el_window = $(id);
		if (!__isWindowOnPage(el_window)) {
			el_window.style.top = "50%";
			el_window.style.left = "50%";
		}
		el_window.style.display = "block";
		__raiseWindow(el_window);
	};

	var __isWindowOnPage = function(el_window) {
		var view_top = $("ctl").clientHeight;
		var view_bottom = Math.max(document.documentElement.clientHeight, window.innerHeight || 0);
		var view_left = 0;
		var view_right = Math.max(document.documentElement.clientWidth, window.innerWidth || 0);

		var rect = el_window.getBoundingClientRect();

		return (
			(rect.bottom - el_window.clientHeight / 1.5) <= view_bottom
			&& rect.top >= view_top
			&& (rect.left + el_window.clientWidth / 1.5) >= view_left
			&& (rect.right - el_window.clientWidth / 1.5) <= view_right
		);
	};

	var __toggleMenu = function(el_a) {
		Array.prototype.forEach.call(document.getElementsByClassName("ctl-item"), function(el_item) {
			var el_menu = el_item.parentElement.querySelector(".ctl-dropdown-content");
			if (el_item === el_a && el_menu.style.display === "none") {
				el_menu.style.display = "block";
				el_item.setAttribute("style", "background-color: var(--bg-color-selected)");
			} else {
				el_menu.style.display = "none";
				el_item.setAttribute("style", "background-color: default");
			}
		});
	};

	var __windowClickHandler = function(event) {
		if (!event.target.matches(".ctl-item")) {
			for (el_item = event.target; el_item && el_item !== document; el_item = el_item.parentNode) {
				if (el_item.hasAttribute("data-force-hide-menu")) {
					break;
				}
				else if (el_item.hasAttribute("data-dont-hide-menu")) {
					return;
				}
			}
			__toggleMenu(null);
		}
	};

	var __makeWindowMovable = function(el_grab, el_window) {
		var prev_x = 0;
		var prev_y = 0;

		function startMoving(event) {
			__raiseWindow(el_window);
			event = (event || window.event);
			event.preventDefault();
			prev_x = event.clientX;
			prev_y = event.clientY;
			document.onmousemove = doMoving;
			document.onmouseup = stopMoving;
		}

		function doMoving(event) {
			event = (event || window.event);
			event.preventDefault();
			x = prev_x - event.clientX;
			y = prev_y - event.clientY;
			prev_x = event.clientX;
			prev_y = event.clientY;
			el_window.style.top = (el_window.offsetTop - y) + "px";
			el_window.style.left = (el_window.offsetLeft - x) + "px";
		}

		function stopMoving() {
			document.onmousemove = null;
			document.onmouseup = null;
		}

		el_grab.onmousedown = startMoving;
		el_window.onclick = function () { __raiseWindow(el_window) };
	};

	var __raiseWindow = function(el_window) {
		__top_z_index += 1;
		el_window.style.zIndex = __top_z_index;
	};
};
