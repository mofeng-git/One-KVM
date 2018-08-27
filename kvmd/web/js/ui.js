function Ui() {
	var self = this;

	/********************************************************************************/

	var __top_z_index = 0;
	var __windows = [];
	var __ctl_items = [];

	var __init__ = function() {
		Array.prototype.forEach.call($$("ctl-item"), function(el_item) {
			el_item.onclick = () => __toggleMenu(el_item);
			__ctl_items.push(el_item);
		});

		Array.prototype.forEach.call($$("window"), function(el_window) {
			__makeWindowMovable(el_window);
			__windows.push(el_window);

			var el_button = el_window.querySelector(".window-header .window-button-close");
			if (el_button) {
				el_button.onclick = function() {
					el_window.style.visibility = "hidden";
					__raiseLastWindow();
				};
			}
		});

		window.onmouseup = __globalMouseButtonHandler;
		// window.oncontextmenu = __globalMouseButtonHandler;

		window.addEventListener("resize", () => __organizeWindowsOnResize(false));
		window.addEventListener("orientationchange", () => __organizeWindowsOnResize(true));

		$("show-about-button").onclick = () => self.showWindow($("about-window"));
		$("show-keyboard-button").onclick = () => self.showWindow($("keyboard-window"));
		$("show-stream-button").onclick = () => self.showWindow($("stream-window"));

		self.showWindow($("stream-window"));
	};

	/********************************************************************************/

	self.error = (...args) => __modalDialog("Error", args.join(" "), true, false);
	self.confirm = (...args) => __modalDialog("Question", args.join(" "), true, true);

	var __modalDialog = function(header, text, ok, cancel) {
		var el_modal = document.createElement("div");
		el_modal.className = "modal";
		el_modal.style.visibility = "visible";

		var el_window = document.createElement("div");
		el_window.className = "modal-window";
		el_window.setAttribute("tabindex", "-1");
		el_modal.appendChild(el_window);

		var el_header = document.createElement("div");
		el_header.className = "modal-header";
		el_header.innerHTML = header;
		el_window.appendChild(el_header);

		var el_content = document.createElement("div");
		el_content.className = "modal-content";
		el_content.innerHTML = text;
		el_window.appendChild(el_content);

		var promise = null;
		if (ok || cancel) {
			promise = new Promise(function(resolve) {
				var el_buttons = document.createElement("div");
				el_buttons.className = "modal-buttons";
				el_window.appendChild(el_buttons);

				var close = function(retval) {
					el_modal.outerHTML = "";
					var index = __windows.indexOf(el_modal);
					if (index !== -1) {
						__windows.splice(index, 1);
					}
					tools.info(__windows);
					__raiseLastWindow();
					resolve(retval);
				};

				if (cancel) {
					var el_cancel_button = document.createElement("button");
					el_cancel_button.innerHTML = "Cancel";
					el_cancel_button.onclick = () => close(false);
					el_buttons.appendChild(el_cancel_button);
				}
				if (ok) {
					var el_ok_button = document.createElement("button");
					el_ok_button.innerHTML = "OK";
					el_ok_button.onclick = () => close(true);
					el_buttons.appendChild(el_ok_button);
				}
				if (ok && cancel) {
					el_ok_button.className = "row50";
					el_cancel_button.className = "row50";
				}
			});
		}

		__windows.push(el_modal);
		document.body.appendChild(el_modal);
		__raiseWindow(el_modal);

		return promise;
	};

	self.showWindow = function(el_window, raise=true) {
		if (!__isWindowOnPage(el_window) || el_window.hasAttribute("data-centered")) {
			var view = __getViewGeometry();
			var rect = el_window.getBoundingClientRect();
			el_window.style.top = Math.max($("ctl").clientHeight, Math.round((view.bottom - rect.height) / 2)) + "px";
			el_window.style.left = Math.round((view.right - rect.width) / 2) + "px";
			el_window.setAttribute("data-centered", "");
		}
		el_window.style.visibility = "visible";
		if (raise) {
			__raiseWindow(el_window);
		}
	};

	var __isWindowOnPage = function(el_window) {
		var view = __getViewGeometry();
		var rect = el_window.getBoundingClientRect();

		return (
			(rect.bottom - el_window.clientHeight / 1.5) <= view.bottom
			&& rect.top >= view.top
			&& (rect.left + el_window.clientWidth / 1.5) >= view.left
			&& (rect.right - el_window.clientWidth / 1.5) <= view.right
		);
	};

	var __getViewGeometry = function() {
		return {
			top: $("ctl").clientHeight,
			bottom: Math.max(document.documentElement.clientHeight, window.innerHeight || 0),
			left: 0,
			right: Math.max(document.documentElement.clientWidth, window.innerWidth || 0),
		};
	};

	var __toggleMenu = function(el_a) {
		var all_hidden = true;

		__ctl_items.forEach(function(el_item) {
			var el_menu = el_item.parentElement.querySelector(".ctl-dropdown-content");
			if (el_item === el_a && window.getComputedStyle(el_menu, null).visibility === "hidden") {
				el_item.classList.add("ctl-item-selected");
				el_menu.style.visibility = "visible";
				all_hidden &= false;
			} else {
				el_item.classList.remove("ctl-item-selected");
				el_menu.style.visibility = "hidden";
			}
		});

		if (all_hidden) {
			document.onkeyup = null;
			__raiseLastWindow();
		} else {
			document.onkeyup = function(event) {
				if (event.code === "Escape") {
					event.preventDefault();
					__closeAllMenues();
					__raiseLastWindow();
				}
			};
		}
	};

	var __closeAllMenues = function() {
		document.onkeyup = null;
		__ctl_items.forEach(function(el_item) {
			var el_menu = el_item.parentElement.querySelector(".ctl-dropdown-content");
			el_item.classList.remove("ctl-item-selected");
			el_menu.style.visibility = "hidden";
		});
	};

	var __globalMouseButtonHandler = function(event) {
		if (!event.target.matches(".ctl-item")) {
			for (var el_item = event.target; el_item && el_item !== document; el_item = el_item.parentNode) {
				if (el_item.hasAttribute("data-force-hide-menu")) {
					break;
				} else if (el_item.hasAttribute("data-dont-hide-menu")) {
					return;
				}
			}
			__closeAllMenues();
			__raiseLastWindow();
		}
	};

	var __organizeWindowsOnResize = function(orientation) {
		var view = __getViewGeometry();
		Array.prototype.forEach.call($$("window"), function(el_window) {
			if (el_window.style.visibility === "visible" && (orientation || el_window.hasAttribute("data-centered"))) {
				var rect = el_window.getBoundingClientRect();
				el_window.style.top = Math.max($("ctl").clientHeight, Math.round((view.bottom - rect.height) / 2)) + "px";
				el_window.style.left = Math.round((view.right - rect.width) / 2) + "px";
				el_window.setAttribute("data-centered", "");
			}
		});
	};

	var __makeWindowMovable = function(el_window) {
		var el_header = el_window.querySelector(".window-header");
		var el_grab = el_window.querySelector(".window-header .window-grab");

		var prev_pos = {x: 0, y: 0};

		function startMoving(event) {
			__closeAllMenues();
			__raiseWindow(el_window);
			event = (event || window.event);
			event.preventDefault();

			if (!event.touches || event.touches.length === 1) {
				el_header.classList.add("window-header-grabbed");

				prev_pos = getEventPosition(event);

				document.onmousemove = doMoving;
				document.onmouseup = stopMoving;

				document.ontouchmove = doMoving;
				document.ontouchend = stopMoving;
			}
		}

		function doMoving(event) {
			el_window.removeAttribute("data-centered");

			event = (event || window.event);
			event.preventDefault();

			var event_pos = getEventPosition(event);
			var x = prev_pos.x - event_pos.x;
			var y = prev_pos.y - event_pos.y;

			el_window.style.top = (el_window.offsetTop - y) + "px";
			el_window.style.left = (el_window.offsetLeft - x) + "px";

			prev_pos = event_pos;
		}

		function stopMoving() {
			el_header.classList.remove("window-header-grabbed");

			document.onmousemove = null;
			document.onmouseup = null;

			document.ontouchmove = null;
			document.ontouchend = null;
		}

		function getEventPosition(event) {
			if (event.touches) {
				return {x: event.touches[0].clientX, y: event.touches[0].clientY};
			} else {
				return {x: event.clientX, y: event.clientY};
			}
		}

		el_window.setAttribute("data-centered", "");
		el_window.onclick = () => __raiseWindow(el_window);

		el_grab.onmousedown = startMoving;
		el_grab.ontouchstart = startMoving;
	};

	var __raiseLastWindow = function() {
		var last_el_window = null;
		var max_z_index = 0;
		__windows.forEach(function(el_window) {
			var z_index = parseInt(window.getComputedStyle(el_window, null).zIndex) || 0;
			if (max_z_index < z_index && window.getComputedStyle(el_window, null).visibility !== "hidden") {
				last_el_window = el_window;
				max_z_index = z_index;
			}
		});
		__raiseWindow(last_el_window);
	};

	var __raiseWindow = function(el_window) {
		if (el_window.className === "modal") {
			el_window.querySelector(".modal-window").focus();
		} else {
			el_window.focus();
		}
		tools.debug("Focused window:", el_window);
		if (el_window.className !== "modal" && parseInt(el_window.style.zIndex) !== __top_z_index) {
			var z_index = __top_z_index + 1;
			el_window.style.zIndex = z_index;
			__top_z_index = z_index;
			tools.debug("Raised window:", el_window);
		}
	};

	__init__();
}
