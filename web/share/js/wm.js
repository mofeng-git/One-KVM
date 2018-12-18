function WindowManager() {
	var self = this;

	/********************************************************************************/

	var __top_z_index = 0;
	var __windows = [];
	var __menu_items = [];

	var __init__ = function() {
		tools.forEach($$$("button"), function(el_button) {
			// XXX: Workaround for iOS Safari:
			// https://stackoverflow.com/questions/3885018/active-pseudo-class-doesnt-work-in-mobile-safari
			el_button.ontouchstart = function() {};
		});

		tools.forEach($$("menu-item"), function(el_item) {
			el_item.parentElement.querySelector(".menu-item-content").setAttribute("tabindex", "-1");
			tools.setOnDown(el_item, () => __toggleMenu(el_item));
			__menu_items.push(el_item);
		});

		tools.forEach($$("window"), function(el_window) {
			el_window.setAttribute("tabindex", "-1");
			__makeWindowMovable(el_window);
			__windows.push(el_window);

			var el_button = el_window.querySelector(".window-header .window-button-close");
			if (el_button) {
				tools.setOnClick(el_button, function() {
					el_window.style.visibility = "hidden";
					__activateLastWindow(el_window);
				});
			}
		});

		window.onmouseup = __globalMouseButtonHandler;
		window.ontouchend = __globalMouseButtonHandler;

		window.addEventListener("focusin", __focusIn);
		window.addEventListener("focusout", __focusOut);

		window.addEventListener("resize", () => __organizeWindowsOnResize(false));
		window.addEventListener("orientationchange", () => __organizeWindowsOnResize(true));
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

				function close(retval) {
					el_window.style.visibility = "hidden";
					el_modal.outerHTML = "";
					var index = __windows.indexOf(el_modal);
					if (index !== -1) {
						__windows.splice(index, 1);
					}
					__activateLastWindow(el_modal);
					resolve(retval);
				}

				if (cancel) {
					var el_cancel_button = document.createElement("button");
					el_cancel_button.innerHTML = "Cancel";
					tools.setOnClick(el_cancel_button, () => close(false));
					el_buttons.appendChild(el_cancel_button);
				}
				if (ok) {
					var el_ok_button = document.createElement("button");
					el_ok_button.innerHTML = "OK";
					tools.setOnClick(el_ok_button, () => close(true));
					el_buttons.appendChild(el_ok_button);
				}
				if (ok && cancel) {
					el_ok_button.className = "row50";
					el_cancel_button.className = "row50";
				}

				el_window.onkeyup = function(event) {
					event.preventDefault();
					if (ok && event.code === "Enter") {
						el_ok_button.click();
					} else if (cancel && event.code === "Escape") {
						el_cancel_button.click();
					}
				};
			});
		}

		__windows.push(el_modal);
		document.body.appendChild(el_modal);
		__activateWindow(el_modal);

		return promise;
	};

	self.switchDisabled = function(el, disabled) {
		if (disabled && document.activeElement === el) {
			var el_to_focus = (
				el.closest(".modal-window")
				|| el.closest(".window")
				|| el.closest(".menu-item-content")
			);
			if (el_to_focus) {
				el_to_focus.focus();
			}
		}
		el.disabled = disabled;
	};

	self.showWindow = function(el_window, activate=true, center=false) {
		if (!__isWindowOnPage(el_window) || el_window.hasAttribute("data-centered") || center) {
			var view = self.getViewGeometry();
			var rect = el_window.getBoundingClientRect();

			el_window.style.top = Math.max(__getMenuHeight(), Math.round((view.bottom - rect.height) / 2)) + "px";
			el_window.style.left = Math.round((view.right - rect.width) / 2) + "px";
			el_window.setAttribute("data-centered", "");
		}

		el_window.style.visibility = "visible";
		if (activate) {
			__activateWindow(el_window);
		}
	};

	self.getViewGeometry = function() {
		return {
			top: __getMenuHeight(),
			bottom: Math.max(document.documentElement.clientHeight, window.innerHeight || 0),
			left: 0,
			right: Math.max(document.documentElement.clientWidth, window.innerWidth || 0),
		};
	};

	var __getMenuHeight = function() {
		var el_menu = $("menu");
		return (el_menu ? el_menu.clientHeight : 0);
	};

	var __isWindowOnPage = function(el_window) {
		var view = self.getViewGeometry();
		var rect = el_window.getBoundingClientRect();

		return (
			(rect.bottom - el_window.clientHeight / 1.5) <= view.bottom
			&& rect.top >= view.top
			&& (rect.left + el_window.clientWidth / 1.5) >= view.left
			&& (rect.right - el_window.clientWidth / 1.5) <= view.right
		);
	};

	var __toggleMenu = function(el_a) {
		var all_hidden = true;

		__menu_items.forEach(function(el_item) {
			var el_menu = el_item.parentElement.querySelector(".menu-item-content");
			if (el_item === el_a && window.getComputedStyle(el_menu, null).visibility === "hidden") {
				el_item.classList.add("menu-item-selected");
				el_menu.style.visibility = "visible";
				el_menu.focus();
				all_hidden &= false;
			} else {
				el_item.classList.remove("menu-item-selected");
				el_menu.style.visibility = "hidden";
			}
		});

		if (all_hidden) {
			document.onkeyup = null;
			__activateLastWindow();
		} else {
			document.onkeyup = function(event) {
				if (event.code === "Escape") {
					event.preventDefault();
					__closeAllMenues();
					__activateLastWindow();
				}
			};
		}
	};

	var __closeAllMenues = function() {
		document.onkeyup = null;
		__menu_items.forEach(function(el_item) {
			var el_menu = el_item.parentElement.querySelector(".menu-item-content");
			el_item.classList.remove("menu-item-selected");
			el_menu.style.visibility = "hidden";
		});
	};

	var __focusIn = function(event) {
		var el_parent;
		if ((el_parent = event.target.closest(".modal-window")) !== null) {
			el_parent.classList.add("window-active");
		} else if ((el_parent = event.target.closest(".window")) !== null) {
			el_parent.classList.add("window-active");
		} else if ((el_parent = event.target.closest(".menu-item-content")) !== null) {
			el_parent.classList.add("menu-item-content-active");
		}
		tools.debug("Focus in:", el_parent);
	};

	var __focusOut = function(event) {
		var el_parent;
		if ((el_parent = event.target.closest(".modal-window")) !== null) {
			el_parent.classList.remove("window-active");
		} else if ((el_parent = event.target.closest(".window")) !== null) {
			el_parent.classList.remove("window-active");
		} else if ((el_parent = event.target.closest(".menu-item-content")) !== null) {
			el_parent.classList.remove("menu-item-content-active");
		}
		tools.debug("Focus out:", el_parent);
	};

	var __globalMouseButtonHandler = function(event) {
		if (!event.target.matches(".menu-item")) {
			for (var el_item = event.target; el_item && el_item !== document; el_item = el_item.parentNode) {
				if (el_item.hasAttribute("data-force-hide-menu")) {
					break;
				} else if (el_item.hasAttribute("data-dont-hide-menu")) {
					return;
				}
			}
			__closeAllMenues();
			__activateLastWindow();
		}
	};

	var __organizeWindowsOnResize = function(orientation) {
		var view = self.getViewGeometry();

		tools.forEach($$("window"), function(el_window) {
			if (el_window.style.visibility === "visible" && (orientation || el_window.hasAttribute("data-centered"))) {
				var rect = el_window.getBoundingClientRect();

				el_window.style.top = Math.max(__getMenuHeight(), Math.round((view.bottom - rect.height) / 2)) + "px";
				el_window.style.left = Math.round((view.right - rect.width) / 2) + "px";
				el_window.setAttribute("data-centered", "");
			}
		});
	};

	var __activateLastWindow = function(el_except_window=null) {
		var el_last_window = null;

		if (document.activeElement) {
			el_last_window = (document.activeElement.closest(".modal-window") || document.activeElement.closest(".window"));
		}

		if (!el_last_window || el_last_window === el_except_window) {
			var max_z_index = 0;

			__windows.forEach(function(el_window) {
				var z_index = parseInt(window.getComputedStyle(el_window, null).zIndex) || 0;
				var visibility = window.getComputedStyle(el_window, null).visibility;

				if (max_z_index < z_index && visibility !== "hidden" && el_window !== el_except_window) {
					el_last_window = el_window;
					max_z_index = z_index;
				}
			});
		}

		if (el_last_window) {
			tools.debug("Activating last window:", el_last_window);
			__activateWindow(el_last_window);
		}
	};

	var __activateWindow = function(el_window) {
		if (window.getComputedStyle(el_window, null).visibility !== "hidden") {
			var el_to_focus;
			var el_window_contains_focus;

			if (el_window.className === "modal") {
				el_to_focus = el_window.querySelector(".modal-window");
				el_window_contains_focus = (document.activeElement && document.activeElement.closest(".modal-window"));
			} else { // .window
				el_to_focus = el_window;
				el_window_contains_focus = (document.activeElement && document.activeElement.closest(".window"));
			}

			if (el_window.className !== "modal" && parseInt(el_window.style.zIndex) !== __top_z_index) {
				__top_z_index += 1;
				el_window.style.zIndex = __top_z_index;
				tools.debug("UI: activated window:", el_window);
			}

			if (el_window !== el_window_contains_focus) {
				el_to_focus.focus();
				tools.debug("UI: focused window:", el_window);
			}
		}
	};

	var __makeWindowMovable = function(el_window) {
		var el_header = el_window.querySelector(".window-header");
		var el_grab = el_window.querySelector(".window-header .window-grab");

		var prev_pos = {x: 0, y: 0};

		function startMoving(event) {
			__closeAllMenues();
			__activateWindow(el_window);
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
		el_window.onclick = el_window.ontouchend = () => __activateWindow(el_window);

		el_grab.onmousedown = startMoving;
		el_grab.ontouchstart = startMoving;
	};

	__init__();
}
