/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
#                                                                            #
#    This program is free software: you can redistribute it and/or modify    #
#    it under the terms of the GNU General Public License as published by    #
#    the Free Software Foundation, either version 3 of the License, or       #
#    (at your option) any later version.                                     #
#                                                                            #
#    This program is distributed in the hope that it will be useful,         #
#    but WITHOUT ANY WARRANTY; without even the implied warranty of          #
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the           #
#    GNU General Public License for more details.                            #
#                                                                            #
#    You should have received a copy of the GNU General Public License       #
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.  #
#                                                                            #
*****************************************************************************/


"use strict";


import {tools, $, $$, $$$} from "./tools.js";


export var wm;

export function initWindowManager() {
	wm = new __WindowManager();
}

function __WindowManager() {
	var self = this;

	/************************************************************************/

	var __top_z_index = 0;
	var __windows = [];
	var __menu_buttons = [];

	var __init__ = function() {
		for (let el_button of $$$("button")) {
			// XXX: Workaround for iOS Safari:
			// https://stackoverflow.com/questions/3885018/active-pseudo-class-doesnt-work-in-mobile-safari
			el_button.ontouchstart = function() {};
		}

		for (let el_button of $$("menu-button")) {
			el_button.parentElement.querySelector(".menu").setAttribute("tabindex", "-1");
			tools.el.setOnDown(el_button, () => __toggleMenu(el_button));
			__menu_buttons.push(el_button);
		}

		if (!window.ResizeObserver) {
			tools.error("ResizeObserver not supported");
		}

		for (let el_window of $$("window")) {
			el_window.setAttribute("tabindex", "-1");
			__makeWindowMovable(el_window);
			__windows.push(el_window);

			if (el_window.classList.contains("window-resizable") && window.ResizeObserver) {
				new ResizeObserver(function() {
					// При переполнении рабочей области сократить размер окна по высоте.
					// По ширине оно настраивается само в CSS.
					let view = self.getViewGeometry();
					let rect = el_window.getBoundingClientRect();
					if ((rect.bottom - rect.top) > (view.bottom - view.top)) {
						let ratio = (rect.bottom - rect.top) / (view.bottom - view.top);
						el_window.style.height = view.bottom - view.top + "px";
						el_window.style.width = Math.round((rect.right - rect.left) / ratio) + "px";
					}

					if (el_window.hasAttribute("data-centered")) {
						__centerWindow(el_window);
					}
				}).observe(el_window);
			}

			let el_close_button = el_window.querySelector(".window-header .window-button-close");
			if (el_close_button) {
				el_close_button.title = "Close window";
				tools.el.setOnClick(el_close_button, () => self.closeWindow(el_window));
			}

			let el_maximize_button = el_window.querySelector(".window-header .window-button-maximize");
			if (el_maximize_button) {
				el_maximize_button.title = "Maximize window";
				tools.el.setOnClick(el_maximize_button, function() {
					__maximizeWindow(el_window);
					__activateLastWindow(el_window);
				});
			}

			let el_orig_button = el_window.querySelector(".window-header .window-button-original");
			if (el_orig_button) {
				el_orig_button.title = "Reduce window to its original size and center it";
				tools.el.setOnClick(el_orig_button, function() {
					el_window.style.width = "";
					el_window.style.height = "";
					__centerWindow(el_window);
					__activateLastWindow(el_window);
				});
			}

			let el_enter_full_tab_button = el_window.querySelector(".window-header .window-button-enter-full-tab");
			let el_exit_full_tab_button = el_window.querySelector(".window-button-exit-full-tab");
			if (el_enter_full_tab_button && el_exit_full_tab_button) {
				el_enter_full_tab_button.title = "Stretch to the entire tab";
				tools.el.setOnClick(el_enter_full_tab_button, () => self.toggleFullTabWindow(el_window, true));
				tools.el.setOnClick(el_exit_full_tab_button, () => self.toggleFullTabWindow(el_window, false));
			}

			let el_full_screen_button = el_window.querySelector(".window-header .window-button-full-screen");
			if (el_full_screen_button && __getFullScreenFunction(el_window)) {
				el_full_screen_button.title = "Go to full-screen mode";
				tools.el.setOnClick(el_full_screen_button, function() {
					__fullScreenWindow(el_window);
					el_window.focus(el_window); // Почему-то теряется фокус
					__activateLastWindow(el_window);
				});
			}
		}

		for (let el_button of $$$("button[data-show-window]")) {
			tools.el.setOnClick(el_button, () => self.showWindow($(el_button.getAttribute("data-show-window"))));
		}

		window.onmouseup = __globalMouseButtonHandler;
		window.ontouchend = __globalMouseButtonHandler;

		window.addEventListener("focusin", (event) => __focusInOut(event, true));
		window.addEventListener("focusout", (event) => __focusInOut(event, false));

		window.addEventListener("resize", __organizeWindowsOnBrowserResize);
		window.addEventListener("orientationchange", __organizeWindowsOnBrowserResize);

		document.onfullscreenchange = __onFullScreenChange;
	};

	/************************************************************************/

	self.copyTextToClipboard = function(text) {
		let workaround = function(err) {
			// https://stackoverflow.com/questions/60317969/document-execcommandcopy-not-working-even-though-the-dom-element-is-created
			let callback = function() {
				tools.error("copyTextToClipboard(): navigator.clipboard.writeText() is not working:", err);
				tools.info("copyTextToClipboard(): Trying a workaround...");

				let el = document.createElement("textarea");
				el.readonly = true;
				el.contentEditable = true;
				el.style.position = "absolute";
				el.style.top = "-1000px";
				el.value = text;
				document.body.appendChild(el);

				// Select the content of the textarea
				el.select(); // Ordinary browsers
				el.setSelectionRange(0, el.value.length); // iOS

				try {
					err = (document.execCommand("copy") ? null : "Unknown error");
				} catch (err) { // eslint-disable-line no-empty
				}

				// Remove the added textarea again:
				document.body.removeChild(el);

				if (err) {
					tools.error("copyTextToClipboard(): Workaround failed:", err);
					wm.error("Can't copy text to the clipboard:<br>", err);
				}
			};
			__modalDialog("Info", "Press OK to copy the text to the clipboard", true, false, callback);
		};
		if (navigator.clipboard) {
			navigator.clipboard.writeText(text).then(function() {
				wm.info("The text has been copied to the clipboard");
			}, function(err) {
				workaround(err);
			});
		} else {
			workaround("navigator.clipboard is not available");
		}
	};

	self.info = (...args) => __modalDialog("Info", args.join(" "), true, false);
	self.error = (...args) => __modalDialog("Error", args.join(" "), true, false);
	self.confirm = (...args) => __modalDialog("Question", args.join(" "), true, true);

	var __modalDialog = function(header, text, ok, cancel, callback=null, parent=null) {
		let el_active_menu = (document.activeElement && document.activeElement.closest(".menu"));

		let el_modal = document.createElement("div");
		el_modal.className = "modal";
		el_modal.style.visibility = "visible";

		let el_window = document.createElement("div");
		el_window.className = "modal-window";
		el_window.setAttribute("tabindex", "-1");
		el_modal.appendChild(el_window);

		let el_header = document.createElement("div");
		el_header.className = "modal-header";
		el_header.innerHTML = header;
		el_window.appendChild(el_header);

		let el_content = document.createElement("div");
		el_content.className = "modal-content";
		el_content.innerHTML = text;
		el_window.appendChild(el_content);

		let promise = null;
		if (ok || cancel) {
			promise = new Promise(function(resolve) {
				let el_buttons = document.createElement("div");
				el_buttons.className = "modal-buttons";
				el_window.appendChild(el_buttons);

				function close(retval) {
					if (callback) {
						callback(retval);
					}
					__closeWindow(el_window);
					el_modal.outerHTML = "";
					let index = __windows.indexOf(el_modal);
					if (index !== -1) {
						__windows.splice(index, 1);
					}
					if (el_active_menu && el_active_menu.style.visibility === "visible") {
						el_active_menu.focus();
					} else {
						__activateLastWindow(el_modal);
					}
					resolve(retval);
				}

				if (cancel) {
					var el_cancel_button = document.createElement("button");
					el_cancel_button.innerHTML = "Cancel";
					tools.el.setOnClick(el_cancel_button, () => close(false));
					el_buttons.appendChild(el_cancel_button);
				}
				if (ok) {
					var el_ok_button = document.createElement("button");
					el_ok_button.innerHTML = "OK";
					tools.el.setOnClick(el_ok_button, () => close(true));
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
		(parent || document.fullscreenElement || document.body).appendChild(el_modal);
		__activateWindow(el_modal);

		return promise;
	};

	self.showWindow = function(el_window, activate=true, center=false) {
		let showed = false;
		if (!self.isWindowVisible(el_window)) {
			center = true;
			showed = true;
		}
		__organizeWindow(el_window, center);
		el_window.style.visibility = "visible";
		if (activate) {
			__activateWindow(el_window);
		}
		if (el_window.show_hook) {
			if (showed) {
				el_window.show_hook();
			}
		}
	};

	self.isWindowVisible = function(el_window) {
		return (window.getComputedStyle(el_window, null).visibility !== "hidden");
	};

	self.getViewGeometry = function() {
		let el_navbar = $("navbar");
		return {
			"top": (el_navbar ? el_navbar.clientHeight : 0), // Navbar height
			"bottom": Math.max(document.documentElement.clientHeight, window.innerHeight || 0),
			"left": 0,
			"right": Math.max(document.documentElement.clientWidth, window.innerWidth || 0),
		};
	};

	self.closeWindow = function(el_window) {
		__closeWindow(el_window);
		__activateLastWindow(el_window);
	};

	self.toggleFullTabWindow = function(el_window, enabled) {
		el_window.classList.toggle("window-full-tab", enabled);
		__activateLastWindow(el_window);
		let el_navbar = $("navbar");
		if (el_navbar) {
			tools.hidden.setVisible(el_navbar, !enabled);
		}
	};

	var __closeWindow = function(el_window) {
		el_window.focus();
		el_window.blur();
		el_window.style.visibility = "hidden";
		if (el_window.close_hook) {
			el_window.close_hook();
		}
	};

	var __toggleMenu = function(el_a) {
		let all_hidden = true;

		for (let el_button of __menu_buttons) {
			let el_menu = el_button.parentElement.querySelector(".menu");
			if (el_button === el_a && window.getComputedStyle(el_menu, null).visibility === "hidden") {
				let rect = el_menu.getBoundingClientRect();
				let offset = self.getViewGeometry().right - (rect.left + el_menu.clientWidth + 2); // + 2 is ugly hack
				if (offset < 0) {
					el_menu.style.right = "0px";
				} else {
					el_menu.style.removeProperty("right");
				}

				el_button.classList.add("menu-button-pressed");
				el_menu.style.visibility = "visible";
				let el_focus = el_menu.querySelector("[data-focus]");
				(el_focus !== null ? el_focus : el_menu).focus();
				all_hidden &= false;
			} else {
				el_button.classList.remove("menu-button-pressed");
				el_menu.style.visibility = "hidden";
				el_menu.style.removeProperty("right");
			}
		}

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
		for (let el_button of __menu_buttons) {
			let el_menu = el_button.parentElement.querySelector(".menu");
			el_button.classList.remove("menu-button-pressed");
			el_menu.style.visibility = "hidden";
			el_menu.style.removeProperty("right");
		}
	};

	var __focusInOut = function(event, focus_in) {
		let el_parent;
		if ((el_parent = event.target.closest(".modal-window")) !== null) {
			el_parent.classList.toggle("window-active", focus_in);
		} else if ((el_parent = event.target.closest(".window")) !== null) {
			el_parent.classList.toggle("window-active", focus_in);
		} else if ((el_parent = event.target.closest(".menu")) !== null) {
			el_parent.classList.toggle("menu-active", focus_in);
		}
		tools.debug(`UI: Focus ${focus_in ? "IN" : "OUT"}:`, el_parent);
	};

	var __globalMouseButtonHandler = function(event) {
		if (
			event.target.closest
			&& !event.target.closest(".menu-button")
			&& !event.target.closest(".modal")
		) {
			for (let el_item = event.target; el_item && el_item !== document; el_item = el_item.parentNode) {
				if (el_item.classList.contains("menu")) {
					return;
				} else if (el_item.hasAttribute("data-force-hide-menu")) {
					break;
				}
			}
			__closeAllMenues();
			__activateLastWindow();
		}
	};

	var __organizeWindowsOnBrowserResize = function() {
		for (let el_window of $$("window")) {
			if (el_window.style.visibility === "visible") {
				if (tools.browser.is_mobile && el_window.classList.contains("window-resizable")) {
					// FIXME: При смене ориентации на мобильном браузере надо сбрасывать
					// настройки окна стрима, поэтому тут стоит вот этот костыль
					el_window.style.width = "";
					el_window.style.height = "";
				}
				__organizeWindow(el_window);
			}
		}
	};

	var __organizeWindow = function(el_window, center=false) {
		let view = self.getViewGeometry();
		let rect = el_window.getBoundingClientRect();

		if (el_window.classList.contains("window-resizable")) {
			// При переполнении рабочей области сократить размер окна
			if ((rect.bottom - rect.top) > (view.bottom - view.top)) {
				let ratio = (rect.bottom - rect.top) / (view.bottom - view.top);
				el_window.style.height = view.bottom - view.top + "px";
				el_window.style.width = Math.round((rect.right - rect.left) / ratio) + "px";
			}
			if ((rect.right - rect.left) > (view.right - view.left)) {
				el_window.style.width = view.right - view.left + "px";
			}
			rect = el_window.getBoundingClientRect();
		}

		if (el_window.hasAttribute("data-centered") || center) {
			__centerWindow(el_window);
		} else {
			if (rect.top <= view.top) {
				el_window.style.top = view.top + "px";
			} else if (rect.bottom > view.bottom) {
				el_window.style.top = view.bottom - rect.height + "px";
			}

			if (rect.left <= view.left) {
				el_window.style.left = view.left + "px";
			} else if (rect.right > view.right) {
				el_window.style.left = view.right - rect.width + "px";
			}
		}
	};

	var __centerWindow = function(el_window) {
		let view = self.getViewGeometry();
		let rect = el_window.getBoundingClientRect();
		el_window.style.top = Math.max(view.top, Math.round((view.bottom - rect.height) / 2)) + "px";
		el_window.style.left = Math.round((view.right - rect.width) / 2) + "px";
		el_window.setAttribute("data-centered", "");
	};

	var __activateLastWindow = function(el_except_window=null) {
		let el_last_window = null;

		if (document.activeElement) {
			el_last_window = (document.activeElement.closest(".modal-window") || document.activeElement.closest(".window"));
			if (el_last_window && window.getComputedStyle(el_last_window, null).visibility === "hidden") {
				el_last_window = null;
			}
		}

		if (!el_last_window || el_last_window === el_except_window) {
			let max_z_index = 0;

			for (let el_window of __windows) {
				let z_index = parseInt(window.getComputedStyle(el_window, null).zIndex) || 0;
				let visibility = window.getComputedStyle(el_window, null).visibility;

				if (max_z_index < z_index && visibility !== "hidden" && el_window !== el_except_window) {
					el_last_window = el_window;
					max_z_index = z_index;
				}
			}
		}

		if (el_last_window) {
			tools.debug("UI: Activating last window:", el_last_window);
			__activateWindow(el_last_window);
		} else {
			tools.debug("UI: No last window to activation");
		}
	};

	var __activateWindow = function(el_window) {
		if (window.getComputedStyle(el_window, null).visibility !== "hidden") {
			let el_to_focus;
			let el_window_contains_focus;

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
				tools.debug("UI: Activated window:", el_window);
			}

			if (el_window !== el_window_contains_focus) {
				el_to_focus.focus();
				tools.debug("UI: Focused window:", el_window);
			}
		}
	};

	var __makeWindowMovable = function(el_window) {
		let el_header = el_window.querySelector(".window-header");
		let el_grab = el_window.querySelector(".window-header .window-grab");
		if (el_header === null || el_grab === null) {
			// Для псевдоокна OCR
			return;
		}

		let prev_pos = {"x": 0, "y": 0};

		function startMoving(event) {
			// При перетаскивании resizable-окна за правый кран экрана оно ужимается.
			// Этот костыль фиксит это.
			el_window.style.width = el_window.offsetWidth + "px";

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

			let event_pos = getEventPosition(event);
			let x = prev_pos.x - event_pos.x;
			let y = prev_pos.y - event_pos.y;

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
				return {"x": event.touches[0].clientX, "y": event.touches[0].clientY};
			} else {
				return {"x": event.clientX, "y": event.clientY};
			}
		}

		el_window.setAttribute("data-centered", "");
		el_window.onmousedown = el_window.ontouchstart = () => __activateWindow(el_window);

		el_grab.onmousedown = startMoving;
		el_grab.ontouchstart = startMoving;
	};

	var __onFullScreenChange = function(event) {
		let el_window = event.target;
		if (!document.fullscreenElement) {
			let rect = el_window.before_full_screen;
			if (rect) {
				el_window.style.width = rect.width + "px";
				el_window.style.height = rect.height + "px";
				el_window.style.top = rect.top + "px";
				el_window.style.left = rect.left + "px";
			}
		}
	};

	var __fullScreenWindow = function(el_window) {
		el_window.before_full_screen = el_window.getBoundingClientRect();
		__getFullScreenFunction(el_window).call(el_window);
		if (navigator.keyboard && navigator.keyboard.lock) {
			navigator.keyboard.lock();
		} else {
			let msg = (
				"Shortcuts like Alt+Tab, Ctrl+W, Ctrl+N might not be captured.<br>"
				+ "For best keyboard handling use any browser with<br><a target=\"_blank\""
				+ " href=\"https://developer.mozilla.org/en-US/docs/Web"
				+ "/API/Keyboard_API#Browser_compatibility\">keyboard lock support from this list</a>.<br><br>"
				+ "In Chrome use HTTPS and enable <i>system-keyboard-lock</i><br>"
				+ "by putting at URL <i>chrome://flags/#system-keyboard-lock</i>"
			);
			__modalDialog("Keyboard lock is unsupported", msg, true, false, null, el_window);
		}
	};

	var __maximizeWindow = function(el_window) {
		let el_navbar = $("navbar");
		let vertical_offset = (el_navbar ? el_navbar.offsetHeight : 0);
		el_window.style.left = "0px";
		el_window.style.top = vertical_offset + "px";
		el_window.style.width = window.innerWidth + "px";
		el_window.style.height = window.innerHeight - vertical_offset + "px";
	};

	var __getFullScreenFunction = function(el_window) {
		if (el_window.requestFullscreen) {
			return el_window.requestFullscreen;
		} else if (el_window.webkitRequestFullscreen) {
			return el_window.webkitRequestFullscreen;
		} else if (el_window.mozRequestFullscreen) {
			return el_window.mozRequestFullscreen;
		}
		return null;
	};

	__init__();
}
