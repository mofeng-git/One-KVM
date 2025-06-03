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

	var __catch_menu_esc = false;

	var __init__ = function() {
		for (let el of $$("menu-button")) {
			el.parentElement.querySelector(".menu").tabIndex = -1;
			tools.el.setOnDown(el, () => __toggleMenu(el));
			__menu_buttons.push(el);
		}

		if (!window.ResizeObserver) {
			tools.error("ResizeObserver not supported");
		}

		for (let el_win of $$("window")) {
			el_win.tabIndex = -1;
			__makeWindowMovable(el_win);
			__windows.push(el_win);

			if (el_win.classList.contains("window-resizable") && window.ResizeObserver) {
				el_win.__observer_timer = null;
				new ResizeObserver(function() {
					// Таймер нужен чтобы остановить дребезг ресайза: observer вызывает
					// __organizeWindow(), который сам по себе триггерит observer.
					if (el_win.__observer_timer === null || el_win.__manual_resizing) {
						__organizeWindow(el_win, !el_win.__manual_resizing);
						if (el_win.__observer_timer !== null) {
							clearTimeout(el_win.__observer_timer);
						}
						el_win.__observer_timer = setTimeout(function() {
							el_win.__observer_timer = null;
						}, 100);
					}
				}).observe(el_win);
				el_win.addEventListener("pointerrawupdate", function(ev) {
					// События pointerdown и touchdown не генерируются при ресайзе за уголок,
					// поэтому отлавливаем pointerrawupdate для тач-событий.
					let events = ev.getCoalescedEvents();
					for (ev of events) {
						if (
							ev.target === el_win && ev.pointerType === "touch" && ev.buttons
							&& Math.abs(el_win.clientWidth - ev.offsetX) < 20
							&& Math.abs(el_win.clientHeight - ev.offsetY) < 20
						) {
							__setWindowMca(el_win, false, null, true);
							break;
						}
					}
				});
				el_win.addEventListener("mousedown", function(ev) {
					if (
						ev.target === el_win
						&& Math.abs(el_win.clientWidth - ev.offsetX) < 20
						&& Math.abs(el_win.clientHeight - ev.offsetY) < 20
					) {
						el_win.__manual_resizing = true;
					}
				});
				document.addEventListener("mouseup", function() {
					if (el_win.__manual_resizing) {
						__organizeWindow(el_win);
					}
					el_win.__manual_resizing = false;
				});
				document.addEventListener("mousemove", function(ev) {
					if (el_win.__manual_resizing) {
						__setWindowMca(el_win, false, null, true);
						if (!ev.buttons) {
							__organizeWindow(el_win);
							el_win.__manual_resizing = false;
						}
					}
				});
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
						__setWindowMca(el_win, true, false, false);
						__organizeWindow(el_win);
						__activateWindow(el_win);
					});
				}
			}

			{
				let el = el_win.querySelector(".window-header .window-button-original");
				if (el) {
					el.title = "Reduce window to its original size and center it";
					tools.el.setOnClick(el, function() {
						__setWindowMca(el_win, false, true, false);
						el_win.style.width = "";
						el_win.style.height = "";
						__organizeWindow(el_win);
						__activateWindow(el_win);
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
				if (el && el_win.requestFullscreen) {
					el.title = "Go to full-screen mode";
					tools.el.setOnClick(el, function() {
						__setFullScreenWindow(el_win);
						__activateWindow(el_win);
					});
				}
			}
		}

		for (let el of $$$("button[data-show-window]")) {
			tools.el.setOnClick(el, () => self.showWindow($(el.getAttribute("data-show-window"))));
		}

		window.addEventListener("mouseup", __globalMouseButtonHandler);
		window.addEventListener("touchend", __globalMouseButtonHandler);

		window.addEventListener("focusin", (ev) => __focusInOut(ev.target, true));
		window.addEventListener("focusout", (ev) => __focusInOut(ev.target, false));

		// Окна с iframe нуждаются в особенной логике для подсветки,
		// потому что из iframe не приходят события фокуса.
		// Мы можешь лишь следить за focus/blur на окне и проверять
		// активный элемент, и если это iframe - назодить его окно,
		// и подсвечивать его. Или наоборот, тушить все окна,
		// в которых есть другие iframe.
		window.addEventListener("focus", function() {
			let el_active = document.activeElement;
			for (let el of document.getElementsByTagName("iframe")) {
				if (el !== el_active) {
					__focusInOut(el, false);
				}
			}
		});
		window.addEventListener("blur", function() {
			// При переходе в iframe, в хромиуме прилетает два блура:
			// с первым активный элемент становится body, со вторым - iframe.
			// В фоксе оба раза это будет body, но если проверить чуть позже -
			// то станет iframe. Таймаут решает проблему.
			setTimeout(function() {
				let el = document.activeElement;
				if (el && el.tagName.toLowerCase() === "iframe") {
					let el_parent = __focusInOut(el, true);
					if (el_parent !== null) {
						__activateWindow(el_parent);
					}
				}
			}, 100);
		});

		window.addEventListener("resize", __organizeWindowsOnBrowserResize);
		window.addEventListener("orientationchange", __organizeWindowsOnBrowserResize);

		document.addEventListener("fullscreenchange", __onFullScreenChange);

		document.addEventListener("keyup", function(ev) {
			if (__catch_menu_esc && ev.code === "Escape") {
				ev.preventDefault();
				__closeAllMenues();
				__activateLastWindow();
			}
		});
	};

	/************************************************************************/

	self.info = (html, ...args) => __modalCodeDialog("Info", html, args.join("\n"), true, false);
	self.error = (html, ...args) => __modalCodeDialog("Error", html, args.join("\n"), true, false);
	self.confirm = (html, ...args) => __modalCodeDialog("Question", html, args.join("\n"), true, true);

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
		return self.modal(header, create_content, ok, cancel);
	};

	self.modal = function(header, html, ok, cancel, save_key=null) {
		let save_id = null;
		if (save_key !== null) {
			save_key = `modal.saved.${save_key}`;
			let saved = tools.storage.getInt(save_key, -1);
			if (saved === 0 || saved === 1) {
				return (new Promise((resolve) => resolve(!!saved)));
			}
		}

		let el_active_menu = (document.activeElement && document.activeElement.closest(".menu"));

		let inner = `
			<div class="modal-window" tabindex="-1">
				<div class="modal-header">${tools.escape(header)}</div>
				<div class="modal-content"></div>
		`;
		if (save_key !== null) {
			save_id = tools.makeTextId();
			inner += `
				<hr style="margin: 0px">
				<div class="modal-content">
					<table style="width: 100%">
						<tr>
							<td>Don't show this message again:</td>
							<td align="right">${tools.sw.makeItem(save_id, false)}</td>
						</tr>
					</table>
				</div>
			`;
		};
		inner += "<div class=\"modal-buttons buttons-row\">";
		let bt_cls = ((ok && cancel) ? "row50": "row100");
		if (cancel) {
			inner += `<button class="modal-button-cancel ${bt_cls}">Cancel</button>`;
		}
		if (ok) {
			inner += `<button class="modal-button-ok ${bt_cls}">OK</button>`;
		}
		inner += "</div></div>";

		let el_modal = document.createElement("div");
		el_modal.className = "modal";
		el_modal.style.visibility = "visible";
		el_modal.innerHTML = inner;

		let el_win = el_modal.querySelector(".modal-window");
		let el_content = el_win.querySelector(".modal-content");
		let el_ok_bt = el_win.querySelector(".modal-button-ok");
		let el_cancel_bt = el_win.querySelector(".modal-button-cancel");

		el_win.addEventListener("keyup", function (ev) {
			ev.preventDefault();
			if (ok && ev.code === "Enter") {
				el_ok_bt.click();
			} else if (cancel && ev.code === "Escape") {
				el_cancel_bt.click();
			}
		});

		let promise = null;
		if (ok || cancel) {
			promise = new Promise(function(resolve) {
				function close(retval) {
					if (save_key !== null && $(save_id).checked) {
						tools.storage.setInt(save_key, (retval ? 1 : 0));
					}

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
		(document.fullscreenElement || document.body).appendChild(el_modal);
		if (typeof html === "function") {
			// Это должно быть здесь, потому что элемент должен иметь родителя чтобы существовать
			html(el_content, el_ok_bt);
		} else {
			el_content.innerHTML = html;
		}
		__activateWindow(el_modal);

		return promise;
	};

	var __setWindowMca = function(el_win, maximized, centered, adjusted) {
		if (maximized !== null) {
			el_win.toggleAttribute("data-maximized", maximized);
			if (maximized) {
				el_win.removeAttribute("data-centered");
			}
		}
		if (centered !== null) {
			el_win.toggleAttribute("data-centered", centered);
			if (centered) {
				el_win.removeAttribute("data-maximized");
			}
		}
		if (adjusted !== null) {
			el_win.toggleAttribute("data-adjusted", adjusted);
			if (adjusted) {
				el_win.removeAttribute("data-maximized");
			}
		}
	};

	self.showWindow = function(el_win) {
		let showed = false;
		if (!self.isWindowVisible(el_win)) {
			showed = true;
		}

		if (!el_win.hasAttribute("data-adjusted")) {
			if (el_win.hasAttribute("data-show-maximized") && !el_win.hasAttribute("data-centered")) {
				__setWindowMca(el_win, true, false, false);
			} else if (el_win.hasAttribute("data-show-centered") && !el_win.hasAttribute("data-maximized")) {
				__setWindowMca(el_win, false, true, false);
			}
		}
		__organizeWindow(el_win);

		el_win.style.visibility = "visible";
		__activateWindow(el_win);
		if (showed && el_win.show_hook) {
			el_win.show_hook();
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
		let el_navbar = $("navbar");
		if (el_navbar) {
			tools.hidden.setVisible(el_navbar, !enabled);
		}
		setTimeout(() => __activateWindow(el_win), 100);
	};

	self.setAspectRatio = function(el_win, width, height) {
		// XXX: Values from CSS
		width += 9 + 9 + 2 + 2;
		height += 30 + 9 + 2 + 2;
		el_win.__aspect_ratio_width = width;
		el_win.__aspect_ratio_height = height;
		el_win.style.maxWidth = "fit-content";
		el_win.style.maxHeight = "fit-content";
		el_win.style.aspectRatio = `${width} / ${height}`;
		__organizeWindow(el_win, true, false);
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
			__catch_menu_esc = false;
			__activateLastWindow();
		} else {
			__catch_menu_esc = true;
		}
	};

	var __closeAllMenues = function() {
		__catch_menu_esc = false;
		for (let el_bt of __menu_buttons) {
			let el_menu = el_bt.parentElement.querySelector(".menu");
			el_bt.classList.remove("menu-button-pressed");
			el_menu.style.visibility = "hidden";
			el_menu.style.removeProperty("right");
		}
	};

	var __focusInOut = function(el, focus_in) {
		let el_parent = null;
		if ((el_parent = el.closest(".modal-window")) !== null) {
			el_parent.classList.toggle("window-active", focus_in);
		} else if ((el_parent = el.closest(".window")) !== null) {
			el_parent.classList.toggle("window-active", focus_in);
		} else if ((el_parent = el.closest(".menu")) !== null) {
			el_parent.classList.toggle("menu-active", focus_in);
		}
		tools.debug(`UI: Focus ${focus_in ? "IN" : "OUT"}:`, el_parent);
		return el_parent;
	};

	var __globalMouseButtonHandler = function(ev) {
		if (
			ev.target.closest
			&& !ev.target.closest(".menu-button")
			&& !ev.target.closest(".modal")
		) {
			for (let el = ev.target; el && el !== document; el = el.parentNode) {
				if (el.classList.contains("menu")) {
					return;
				} else if (el.hasAttribute("data-force-hide-menu")) {
					break;
				}
			}
			setTimeout(function() {
				// Тач-событие на хроме не долетает при data-force-hide-menu,
				// судя по всему оно прерывается при закрытии меню.
				// Откладываем обработку.
				__closeAllMenues();
				__activateLastWindow();
			}, 10);
		}
	};

	var __organizeWindowsOnBrowserResize = function() {
		for (let el_win of $$("window")) {
			if (el_win.style.visibility === "visible") {
				__organizeWindow(el_win);
			}
		}
	};

	var __organizeWindow = function(el_win, auto_shrink=true, organize_hook=true) {
		if (organize_hook && el_win.organize_hook) {
			el_win.organize_hook();
		}

		if (auto_shrink && el_win.classList.contains("window-resizable")) {
			// При переполнении рабочей области сократить размер окна
			let view = self.getViewGeometry();
			let rect = el_win.getBoundingClientRect();
			if ((rect.bottom - rect.top) > (view.bottom - view.top)) {
				let ratio = (rect.bottom - rect.top) / (view.bottom - view.top);
				el_win.style.height = view.bottom - view.top + "px";
				el_win.style.width = Math.round((rect.right - rect.left) / ratio) + "px";
			}
			if ((rect.right - rect.left) > (view.right - view.left)) {
				el_win.style.width = view.right - view.left + "px";
			}
		}

		if (el_win.hasAttribute("data-maximized")) {
			__organizeMaximizeWindow(el_win);
		} else if (el_win.hasAttribute("data-centered")) {
			__organizeCenterWindow(el_win);
		} else {
			__organizeFitWindow(el_win);
		}
	};

	var __organizeCenterWindow = function(el_win) {
		let view = self.getViewGeometry();
		let rect = el_win.getBoundingClientRect();
		el_win.style.top = Math.max(view.top, Math.round((view.bottom - rect.height) / 2)) + "px";
		el_win.style.left = Math.round((view.right - rect.width) / 2) + "px";
	};

	var __organizeMaximizeWindow = function(el_win) {
		let view = self.getViewGeometry();
		el_win.style.top = view.top + "px";

		let aw = el_win.__aspect_ratio_width;
		let ah = el_win.__aspect_ratio_height;
		let gw = view.right - view.left;
		let gh = view.bottom - view.top;
		if (aw && ah) {
			// Умная машинерия только для aspect-ratio
			if (aw / gw < ah / gh) {
				el_win.style.width = "";
				el_win.style.height = gh + "px";
			} else {
				el_win.style.left = "";
				el_win.style.height = "";
				el_win.style.width = gw + "px";
			}
		} else if (!el_win.hasAttribute("data-organize-hook")) {
			// FIXME: Можно было бы проверять наличие organize_hook,
			// но эвент от обзервера приходит раньше чем настроятся хуки.
			// По идее это надо бы глобально исправить.
			el_win.style.width = gw + "px";
			el_win.style.height = gh + "px";
		}

		let rect = el_win.getBoundingClientRect();
		el_win.style.left = Math.round((view.right - rect.width) / 2) + "px";
	};

	var __organizeFitWindow = function(el_win) {
		let view = self.getViewGeometry();
		let rect = el_win.getBoundingClientRect();

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
	};

	var __activateLastWindow = function(el_except_win=null) {
		let el_last_win = null;

		let el_active = document.activeElement;
		if (el_active) {
			el_last_win = (el_active.closest(".modal-window") || el_active.closest(".window"));
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

			let el_active = document.activeElement;
			if (el_win.className === "modal") {
				el_to_focus = el_win.querySelector(".modal-window");
				el_focused = (el_active && el_active.closest(".modal-window"));
			} else { // .window
				el_to_focus = el_win;
				el_focused = (el_active && el_active.closest(".window"));
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
		let moving = false;

		function startMoving(ev) {
			// При перетаскивании resizable-окна за правый кран экрана оно ужимается.
			// Этот костыль фиксит это.
			el_win.style.width = el_win.offsetWidth + "px";

			__closeAllMenues();
			__activateWindow(el_win);
			ev = (ev || window.ev);
			ev.preventDefault();

			if (!ev.touches || ev.touches.length === 1) {
				el_header.classList.add("window-header-grabbed");
				prev_pos = getEventPosition(ev);
				moving = true;
			}
		}

		function doMoving(ev) {
			if (!moving) {
				return;
			}

			__setWindowMca(el_win, false, false, true);

			ev = (ev || window.ev);
			ev.preventDefault();

			let ev_pos = getEventPosition(ev);
			let x = prev_pos.x - ev_pos.x;
			let y = prev_pos.y - ev_pos.y;

			el_win.style.top = (el_win.offsetTop - y) + "px";
			el_win.style.left = (el_win.offsetLeft - x) + "px";

			prev_pos = ev_pos;
		}

		function stopMoving() {
			el_header.classList.remove("window-header-grabbed");
			moving = false;
		}

		function getEventPosition(ev) {
			if (ev.touches) {
				return {"x": ev.touches[0].clientX, "y": ev.touches[0].clientY};
			} else {
				return {"x": ev.clientX, "y": ev.clientY};
			}
		}

		document.addEventListener("mousemove", doMoving);
		document.addEventListener("mouseup", stopMoving);

		document.addEventListener("touchmove", doMoving);
		document.addEventListener("touchend", stopMoving);

		el_win.addEventListener("mousedown", () => __activateWindow(el_win));
		el_win.addEventListener("touchstart", () => __activateWindow(el_win));

		el_grab.addEventListener("mousedown", startMoving);
		el_grab.addEventListener("touchstart", startMoving);
	};

	var __onFullScreenChange = function(ev) {
		let el_win = ev.target;
		if (!document.fullscreenElement) {
			let rect = el_win.__before_full_screen_rect;
			if (rect) {
				el_win.style.width = rect.width + "px";
				el_win.style.height = rect.height + "px";
				el_win.style.top = rect.top + "px";
				el_win.style.left = rect.left + "px";
			}
		}
	};

	var __setFullScreenWindow = function(el_win) {
		el_win.__before_full_screen_rect = el_win.getBoundingClientRect();
		el_win.requestFullscreen().then(function() {
			el_win.focus(el_win); // Почему-то теряется фокус
			if (navigator.keyboard && navigator.keyboard.lock) {
				navigator.keyboard.lock();
			} else {
				let html = (
					"Shortcuts like Alt+Tab and Ctrl+W might not be captured.<br>"
					+ "For best keyboard handling use any browser with<br><a target=\"_blank\""
					+ " href=\"https://developer.mozilla.org/en-US/docs/Web"
					+ "/API/Keyboard_API#Browser_compatibility\">keyboard lock support from this list</a>.<br><br>"
					+ "In Chrome use HTTPS and enable <i>system-keyboard-lock</i><br>"
					+ "by putting at URL <i>chrome://flags/#system-keyboard-lock</i>"
				);
				self.modal("The Keyboard Lock API is not supported", html, true, false, "full-screen");
			}
		});
	};

	__init__();
}
