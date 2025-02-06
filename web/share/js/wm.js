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
		for (let el of $$$("button")) {
			// XXX: Workaround for iOS Safari:
			// https://stackoverflow.com/questions/3885018/active-pseudo-class-doesnt-work-in-mobile-safari
			el.ontouchstart = function() {};
		}

		for (let el of $$("menu-button")) {
			el.parentElement.querySelector(".menu").setAttribute("tabindex", "-1");
			tools.el.setOnDown(el, () => __toggleMenu(el));
			__menu_buttons.push(el);
		}

		if (!window.ResizeObserver) {
			tools.error("ResizeObserver not supported");
		}

		for (let el_win of $$("window")) {
			el_win.setAttribute("tabindex", "-1");
			__makeWindowMovable(el_win);
			__windows.push(el_win);

			if (el_win.classList.contains("window-resizable") && window.ResizeObserver) {
				new ResizeObserver(function() {
					// При переполнении рабочей области сократить размер окна по высоте.
					// По ширине оно настраивается само в CSS.
					let view = self.getViewGeometry();
					let rect = el_win.getBoundingClientRect();
					if ((rect.bottom - rect.top) > (view.bottom - view.top)) {
						let ratio = (rect.bottom - rect.top) / (view.bottom - view.top);
						el_win.style.height = view.bottom - view.top + "px";
						el_win.style.width = Math.round((rect.right - rect.left) / ratio) + "px";
					}

					if (el_win.hasAttribute("data-centered")) {
						__centerWindow(el_win);
					}
				}).observe(el_win);
			}

			{
				let el = el_win.querySelector(".window-header .window-button-close");
				if (el) {
					el.title = "Close window";
					tools.el.setOnClick(el, () => self.closeWindow(el_win));
				}
			}

			{
				let el = el_win.querySelector(".window-header .window-button-maximize");
				if (el) {
					el.title = "Maximize window";
					tools.el.setOnClick(el, function() {
						__maximizeWindow(el_win);
						__activateLastWindow(el_win);
					});
				}
			}

			{
				let el = el_win.querySelector(".window-header .window-button-original");
				if (el) {
					el.title = "Reduce window to its original size and center it";
					tools.el.setOnClick(el, function() {
						el_win.style.width = "";
						el_win.style.height = "";
						__centerWindow(el_win);
						__activateLastWindow(el_win);
					});
				}
			}

			{
				let el_enter = el_win.querySelector(".window-header .window-button-enter-full-tab");
				let el_exit = el_win.querySelector(".window-button-exit-full-tab");
				if (el_enter && el_exit) {
					el_enter.title = "Stretch to the entire tab";
					tools.el.setOnClick(el_enter, () => self.setFullTabWindow(el_win, true));
					tools.el.setOnClick(el_exit, () => self.setFullTabWindow(el_win, false));
				}
			}

			{
				let el = el_win.querySelector(".window-header .window-button-full-screen");
				if (el && __getFullScreenFunction(el_win)) {
					el.title = "Go to full-screen mode";
					tools.el.setOnClick(el, function() {
						__fullScreenWindow(el_win);
						el_win.focus(el_win); // Почему-то теряется фокус
						__activateLastWindow(el_win);
					});
				}
			}
		}

		for (let el of $$$("button[data-show-window]")) {
			tools.el.setOnClick(el, () => self.showWindow($(el.getAttribute("data-show-window"))));
		}

		window.onmouseup = window.ontouchend = __globalMouseButtonHandler;

		window.addEventListener("focusin", (event) => __focusInOut(event, true));
		window.addEventListener("focusout", (event) => __focusInOut(event, false));

		window.addEventListener("resize", __organizeWindowsOnBrowserResize);
		window.addEventListener("orientationchange", __organizeWindowsOnBrowserResize);

		document.onfullscreenchange = __onFullScreenChange;
	};

	/************************************************************************/

	self.copyTextToClipboard = function(text) {
		let workaround = function(ex) {
			// https://stackoverflow.com/questions/60317969/document-execcommandcopy-not-working-even-though-the-dom-element-is-created
			__modalDialog("Info", "Press OK to copy the text to the clipboard", true, false).then(function() {
				tools.error("copyTextToClipboard(): navigator.clipboard.writeText() is not working:", ex);
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
					ex = (document.execCommand("copy") ? null : "Unknown error");
				} catch (ex) { // eslint-disable-line no-unused-vars
				}

				// Remove the added textarea again:
				document.body.removeChild(el);

				if (ex) {
					tools.error("copyTextToClipboard(): Workaround failed:", ex);
					self.error("Can't copy text to the clipboard", `${ex}`);
				}
			});
		};
		if (navigator.clipboard) {
			navigator.clipboard.writeText(text).then(function() {
				self.info("The text has been copied to the clipboard");
			}, function(ex) {
				workaround(ex);
			});
		} else {
			workaround("navigator.clipboard is not available");
		}
	};

	self.info = (html, ...args) => __modalCodeDialog("Info", html, args.join("\n"), true, false);
	self.error = (html, ...args) => __modalCodeDialog("Error", html, args.join("\n"), true, false);
	self.confirm = (html, ...args) => __modalCodeDialog("Question", html, args.join("\n"), true, true);
	self.modal = (header, html, ok, cancel) => __modalDialog(header, html, ok, cancel);

	var __modalCodeDialog = function(header, html, code, ok, cancel) {
		let create_content = function(el_content) {
			if (code) {
				html += `
					<br><br>
					<div class="code">
						<pre style="margin:0px">${tools.escape(code)}</pre>
					</div>
				`;
			}
			el_content.innerHTML = html;
		};
		return __modalDialog(header, create_content, ok, cancel);
	};

	var __modalDialog = function(header, html, ok, cancel, parent=null) {
		let el_active_menu = (document.activeElement && document.activeElement.closest(".menu"));

		let el_modal = document.createElement("div");
		el_modal.className = "modal";
		el_modal.style.visibility = "visible";

		let el_win = document.createElement("div");
		el_win.className = "modal-window";
		el_win.setAttribute("tabindex", "-1");
		el_modal.appendChild(el_win);

		let el_header = document.createElement("div");
		el_header.className = "modal-header";
		el_header.innerText = header;
		el_win.appendChild(el_header);

		let el_content = document.createElement("div");
		el_content.className = "modal-content";
		el_win.appendChild(el_content);

		let el_buttons = document.createElement("div");
		el_buttons.classList.add("modal-buttons", "buttons-row");
		el_win.appendChild(el_buttons);

		let el_cancel_bt = null;
		let el_ok_bt = null;
		if (cancel) {
			el_cancel_bt = document.createElement("button");
			el_cancel_bt.className = "row100";
			el_cancel_bt.innerText = "Cancel";
			el_buttons.appendChild(el_cancel_bt);
		}
		if (ok) {
			el_ok_bt = document.createElement("button");
			el_ok_bt.className = "row100";
			el_ok_bt.innerText = "OK";
			el_buttons.appendChild(el_ok_bt);
		}
		if (ok && cancel) {
			el_ok_bt.className = "row50";
			el_cancel_bt.className = "row50";
		}

		el_win.onkeyup = function(event) {
			event.preventDefault();
			if (ok && event.code === "Enter") {
				el_ok_bt.click();
			} else if (cancel && event.code === "Escape") {
				el_cancel_bt.click();
			}
		};

		let promise = null;
		if (ok || cancel) {
			promise = new Promise(function(resolve) {
				function close(retval) {
					__closeWindow(el_win);
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
					// Так как resolve() асинхронный, надо выполнить в эвентлупе после него
					setTimeout(function() { el_modal.outerHTML = ""; }, 0);
				}

				if (cancel) {
					tools.el.setOnClick(el_cancel_bt, () => close(false));
				}
				if (ok) {
					tools.el.setOnClick(el_ok_bt, () => close(true));
				}
			});
		}

		__windows.push(el_modal);
		(parent || document.fullscreenElement || document.body).appendChild(el_modal);
		if (typeof html === "function") {
			// Это должно быть здесь, потому что элемент должен иметь родителя чтобы существовать
			html(el_content, el_ok_bt);
		} else {
			el_content.innerHTML = html;
		}
		__activateWindow(el_modal);

		return promise;
	};

	self.showWindow = function(el_win, activate=true, center=false) {
		let showed = false;
		if (!self.isWindowVisible(el_win)) {
			center = true;
			showed = true;
		}
		__organizeWindow(el_win, center);
		el_win.style.visibility = "visible";
		if (activate) {
			__activateWindow(el_win);
		}
		if (el_win.show_hook) {
			if (showed) {
				el_win.show_hook();
			}
		}
	};

	self.isWindowVisible = function(el_win) {
		return (window.getComputedStyle(el_win, null).visibility !== "hidden");
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

	self.closeWindow = function(el_win) {
		__closeWindow(el_win);
		__activateLastWindow(el_win);
	};

	self.setFullTabWindow = function(el_win, enabled) {
		el_win.classList.toggle("window-full-tab", enabled);
		__activateLastWindow(el_win);
		let el_navbar = $("navbar");
		if (el_navbar) {
			tools.hidden.setVisible(el_navbar, !enabled);
		}
	};

	var __closeWindow = function(el_win) {
		el_win.focus();
		el_win.blur();
		el_win.style.visibility = "hidden";
		if (el_win.close_hook) {
			el_win.close_hook();
		}
	};

	var __toggleMenu = function(el_a) {
		let all_hidden = true;

		for (let el_bt of __menu_buttons) {
			let el_menu = el_bt.parentElement.querySelector(".menu");
			if (el_bt === el_a && window.getComputedStyle(el_menu, null).visibility === "hidden") {
				let rect = el_menu.getBoundingClientRect();
				let offset = self.getViewGeometry().right - (rect.left + el_menu.clientWidth + 2); // + 2 is ugly hack
				if (offset < 0) {
					el_menu.style.right = "0px";
				} else {
					el_menu.style.removeProperty("right");
				}

				el_bt.classList.add("menu-button-pressed");
				el_menu.style.visibility = "visible";
				let el_focus = el_menu.querySelector("[data-focus]");
				(el_focus !== null ? el_focus : el_menu).focus();
				all_hidden &= false;
			} else {
				el_bt.classList.remove("menu-button-pressed");
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
		for (let el_bt of __menu_buttons) {
			let el_menu = el_bt.parentElement.querySelector(".menu");
			el_bt.classList.remove("menu-button-pressed");
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
			for (let el = event.target; el && el !== document; el = el.parentNode) {
				if (el.classList.contains("menu")) {
					return;
				} else if (el.hasAttribute("data-force-hide-menu")) {
					break;
				}
			}
			__closeAllMenues();
			__activateLastWindow();
		}
	};

	var __organizeWindowsOnBrowserResize = function() {
		for (let el_win of $$("window")) {
			if (el_win.style.visibility === "visible") {
				if (tools.browser.is_mobile && el_win.classList.contains("window-resizable")) {
					// FIXME: При смене ориентации на мобильном браузере надо сбрасывать
					// настройки окна стрима, поэтому тут стоит вот этот костыль
					el_win.style.width = "";
					el_win.style.height = "";
				}
				__organizeWindow(el_win);
			}
		}
	};

	var __organizeWindow = function(el_win, center=false) {
		let view = self.getViewGeometry();
		let rect = el_win.getBoundingClientRect();

		if (el_win.classList.contains("window-resizable")) {
			// При переполнении рабочей области сократить размер окна
			if ((rect.bottom - rect.top) > (view.bottom - view.top)) {
				let ratio = (rect.bottom - rect.top) / (view.bottom - view.top);
				el_win.style.height = view.bottom - view.top + "px";
				el_win.style.width = Math.round((rect.right - rect.left) / ratio) + "px";
			}
			if ((rect.right - rect.left) > (view.right - view.left)) {
				el_win.style.width = view.right - view.left + "px";
			}
			rect = el_win.getBoundingClientRect();
		}

		if (el_win.hasAttribute("data-centered") || center) {
			__centerWindow(el_win);
		} else {
			if (rect.top <= view.top) {
				el_win.style.top = view.top + "px";
			} else if (rect.bottom > view.bottom) {
				el_win.style.top = view.bottom - rect.height + "px";
			}

			if (rect.left <= view.left) {
				el_win.style.left = view.left + "px";
			} else if (rect.right > view.right) {
				el_win.style.left = view.right - rect.width + "px";
			}
		}
	};

	var __centerWindow = function(el_win) {
		let view = self.getViewGeometry();
		let rect = el_win.getBoundingClientRect();
		el_win.style.top = Math.max(view.top, Math.round((view.bottom - rect.height) / 2)) + "px";
		el_win.style.left = Math.round((view.right - rect.width) / 2) + "px";
		el_win.setAttribute("data-centered", "");
	};

	var __activateLastWindow = function(el_except_win=null) {
		let el_last_win = null;

		if (document.activeElement) {
			el_last_win = (document.activeElement.closest(".modal-window") || document.activeElement.closest(".window"));
			if (el_last_win && window.getComputedStyle(el_last_win, null).visibility === "hidden") {
				el_last_win = null;
			}
		}

		if (!el_last_win || el_last_win === el_except_win) {
			let max_z_index = 0;

			for (let el_win of __windows) {
				let z_index = parseInt(window.getComputedStyle(el_win, null).zIndex) || 0;
				let visibility = window.getComputedStyle(el_win, null).visibility;

				if (max_z_index < z_index && visibility !== "hidden" && el_win !== el_except_win) {
					el_last_win = el_win;
					max_z_index = z_index;
				}
			}
		}

		if (el_last_win) {
			tools.debug("UI: Activating last window:", el_last_win);
			__activateWindow(el_last_win);
		} else {
			tools.debug("UI: No last window to activation");
		}
	};

	var __activateWindow = function(el_win) {
		if (window.getComputedStyle(el_win, null).visibility !== "hidden") {
			let el_to_focus;
			let el_focused; // A window which contains a focus

			if (el_win.className === "modal") {
				el_to_focus = el_win.querySelector(".modal-window");
				el_focused = (document.activeElement && document.activeElement.closest(".modal-window"));
			} else { // .window
				el_to_focus = el_win;
				el_focused = (document.activeElement && document.activeElement.closest(".window"));
			}

			if (el_win.className !== "modal" && parseInt(el_win.style.zIndex) !== __top_z_index) {
				__top_z_index += 1;
				el_win.style.zIndex = __top_z_index;
				tools.debug("UI: Activated window:", el_win);
			}

			if (el_win !== el_focused) {
				el_to_focus.focus();
				tools.debug("UI: Focused window:", el_win);
			}
		}
	};

	var __makeWindowMovable = function(el_win) {
		let el_header = el_win.querySelector(".window-header");
		let el_grab = el_win.querySelector(".window-header .window-grab");
		if (el_header === null || el_grab === null) {
			// Для псевдоокна OCR
			return;
		}

		let prev_pos = {"x": 0, "y": 0};

		function startMoving(event) {
			// При перетаскивании resizable-окна за правый кран экрана оно ужимается.
			// Этот костыль фиксит это.
			el_win.style.width = el_win.offsetWidth + "px";

			__closeAllMenues();
			__activateWindow(el_win);
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
			el_win.removeAttribute("data-centered");

			event = (event || window.event);
			event.preventDefault();

			let event_pos = getEventPosition(event);
			let x = prev_pos.x - event_pos.x;
			let y = prev_pos.y - event_pos.y;

			el_win.style.top = (el_win.offsetTop - y) + "px";
			el_win.style.left = (el_win.offsetLeft - x) + "px";

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

		el_win.setAttribute("data-centered", "");
		el_win.onmousedown = el_win.ontouchstart = () => __activateWindow(el_win);

		el_grab.onmousedown = startMoving;
		el_grab.ontouchstart = startMoving;
	};

	var __onFullScreenChange = function(event) {
		let el_win = event.target;
		if (!document.fullscreenElement) {
			let rect = el_win.before_full_screen;
			if (rect) {
				el_win.style.width = rect.width + "px";
				el_win.style.height = rect.height + "px";
				el_win.style.top = rect.top + "px";
				el_win.style.left = rect.left + "px";
			}
		}
	};

	var __fullScreenWindow = function(el_win) {
		el_win.before_full_screen = el_win.getBoundingClientRect();
		__getFullScreenFunction(el_win).call(el_win);
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
			__modalDialog("Keyboard lock is unsupported", msg, true, false, el_win);
		}
	};

	var __maximizeWindow = function(el_win) {
		let el_navbar = $("navbar");
		let vertical_offset = (el_navbar ? el_navbar.offsetHeight : 0);
		el_win.style.left = "0px";
		el_win.style.top = vertical_offset + "px";
		el_win.style.width = window.innerWidth + "px";
		el_win.style.height = window.innerHeight - vertical_offset + "px";
	};

	var __getFullScreenFunction = function(el_win) {
		if (el_win.requestFullscreen) {
			return el_win.requestFullscreen;
		} else if (el_win.webkitRequestFullscreen) {
			return el_win.webkitRequestFullscreen;
		} else if (el_win.mozRequestFullscreen) {
			return el_win.mozRequestFullscreen;
		}
		return null;
	};

	__init__();
}
