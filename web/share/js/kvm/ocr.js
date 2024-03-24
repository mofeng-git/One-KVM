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


import {tools, $} from "../tools.js";
import {wm} from "../wm.js";


export function Ocr(__getGeometry) {
	var self = this;

	/************************************************************************/

	var __start_pos = null;
	var __end_pos = null;
	var __selection = null;

	var __init__ = function() {
		tools.el.setOnClick($("stream-ocr-button"), function() {
			__resetSelection();
			wm.showWindow($("stream-window"));
			wm.showWindow($("stream-ocr-window"));
		});

		$("stream-ocr-lang-selector").addEventListener("change", function() {
			tools.storage.set("stream.ocr.lang", $("stream-ocr-lang-selector").value);
		});

		$("stream-ocr-window").addEventListener("blur", __resetSelection);
		$("stream-ocr-window").addEventListener("resize", __resetSelection);
		$("stream-ocr-window").close_hook = __resetSelection;

		$("stream-ocr-window").onkeyup = function(event) {
			event.preventDefault();
			if (event.code === "Enter") {
				if (__selection) {
					__recognizeSelection();
					wm.closeWindow($("stream-ocr-window"));
				}
			} else if (event.code === "Escape") {
				wm.closeWindow($("stream-ocr-window"));
			}
		};

		$("stream-ocr-window").onmousedown = __startSelection;
		$("stream-ocr-window").onmousemove = __changeSelection;
		$("stream-ocr-window").onmouseup = __endSelection;
	};

	/************************************************************************/

	self.setState = function(state) {
		let enabled = (state && state.ocr.enabled && !tools.browser.is_mobile);
		if (enabled) {
			let el = $("stream-ocr-lang-selector");
			tools.selector.setValues(el, state.ocr.langs.available);
			tools.selector.setSelectedValue(el, tools.storage.get("stream.ocr.lang", state.ocr.langs["default"]));
		}
		tools.feature.setEnabled($("stream-ocr"), enabled);
		$("stream-ocr-led").className = (enabled ? "led-gray" : "hidden");
	};

	var __startSelection = function(event) {
		if (__start_pos === null) {
			tools.hidden.setVisible($("stream-ocr-selection"), false);
			__start_pos = __getGlobalPosition(event);
			__end_pos = null;
		}
	};

	var __changeSelection = function(event) {
		if (__start_pos !== null) {
			__end_pos = __getGlobalPosition(event);
			let width = Math.abs(__start_pos.x - __end_pos.x);
			let height = Math.abs(__start_pos.y - __end_pos.y);
			let el_selection = $("stream-ocr-selection");
			el_selection.style.left = Math.min(__start_pos.x, __end_pos.x) + "px";
			el_selection.style.top = Math.min(__start_pos.y, __end_pos.y) + "px";
			el_selection.style.width = width + "px";
			el_selection.style.height = height + "px";
			tools.hidden.setVisible(el_selection, (width > 1 || height > 1));
		}
	};

	var __endSelection = function(event) {
		__changeSelection(event);
		let el_selection = $("stream-ocr-selection");
		let ok = (
			el_selection.offsetWidth > 1 && el_selection.offsetHeight > 1
			&& __start_pos !== null && __end_pos !== null
		);
		tools.hidden.setVisible(el_selection, ok);
		if (ok) {
			let rect = $("stream-box").getBoundingClientRect();
			let rel_left = Math.min(__start_pos.x, __end_pos.x) - rect.left;
			let rel_right = Math.max(__start_pos.x, __end_pos.x) - rect.left;
			let offset = __getNavbarOffset();
			let rel_top = Math.min(__start_pos.y, __end_pos.y) - rect.top + offset;
			let rel_bottom = Math.max(__start_pos.y, __end_pos.y) - rect.top + offset;
			let geo = __getGeometry();
			__selection = {
				"left": tools.remap(rel_left, geo.x, geo.width, 0, geo.real_width),
				"right": tools.remap(rel_right, geo.x, geo.width, 0, geo.real_width),
				"top": tools.remap(rel_top, geo.y, geo.height, 0, geo.real_height),
				"bottom": tools.remap(rel_bottom, geo.y, geo.height, 0, geo.real_height),
			};
		} else {
			__selection = null;
		}
		__start_pos = null;
		__end_pos = null;
	};

	var __getGlobalPosition = function(event) {
		let rect = $("stream-box").getBoundingClientRect();
		let geo = __getGeometry();
		let offset = __getNavbarOffset();
		return {
			"x": Math.min(Math.max(event.clientX, rect.left + geo.x), rect.right - geo.x),
			"y": Math.min(Math.max(event.clientY - offset, rect.top + geo.y - offset), rect.bottom - geo.y - offset),
		};
	};

	var __getNavbarOffset = function() {
		if (tools.browser.is_firefox) {
			// На лисе наблюдается оффсет из-за навбара, хз почему
			return wm.getViewGeometry().top;
		}
		return 0;
	};

	var __resetSelection = function() {
		tools.hidden.setVisible($("stream-ocr-selection"), false);
		__start_pos = null;
		__end_pos = null;
		__selection = null;
	};

	var __recognizeSelection = function() {
		tools.el.setEnabled($("stream-ocr-button"), false);
		tools.el.setEnabled($("stream-ocr-lang-selector"), false);
		$("stream-ocr-led").className = "led-yellow-rotating-fast";

		let lang = $("stream-ocr-lang-selector").value;
		let url = `/api/streamer/snapshot?ocr=1&ocr_langs=${lang}`;
		url += `&ocr_left=${__selection.left}&ocr_top=${__selection.top}`;
		url += `&ocr_right=${__selection.right}&ocr_bottom=${__selection.bottom}`;

		tools.httpGet(url, function(http) {
			if (http.status === 200) {
				wm.copyTextToClipboard(http.responseText);
			} else {
				wm.error("OCR error:<br>", http.responseText);
			}
			tools.el.setEnabled($("stream-ocr-button"), true);
			tools.el.setEnabled($("stream-ocr-lang-selector"), true);
			$("stream-ocr-led").className = "led-gray";
		}, null, null, 30000);
	};

	__init__();
}
