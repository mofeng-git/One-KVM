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


import {browser} from "./bb.js";


export var tools = new function() {
	var self = this;

	/************************************************************************/

	self.setDefault = function(dict, key, value) {
		if (!(key in dict)) {
			dict[key] = value;
		}
	};

	/************************************************************************/

	self.httpRequest = function(method, url, callback, body=null, content_type=null, timeout=15000) {
		let http = new XMLHttpRequest();
		http.open(method, url, true);
		if (content_type) {
			http.setRequestHeader("Content-Type", content_type);
		}
		http.onreadystatechange = function() {
			if (http.readyState === 4) {
				callback(http);
			}
		};
		http.timeout = timeout;
		http.send(body);
	};

	self.httpGet = function(url, callback, body=null, content_type=null, timeout=15000) {
		self.httpRequest("GET", url, callback, body, content_type, timeout);
	};

	self.httpPost = function(url, callback, body=null, content_type=null, timeout=15000) {
		self.httpRequest("POST", url, callback, body, content_type, timeout);
	};

	/************************************************************************/

	self.upperFirst = function(text) {
		return text[0].toUpperCase() + text.slice(1);
	};

	self.makeId = function() {
		let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
		let id = "";
		for (let count = 0; count < 16; ++count) {
			id += chars.charAt(Math.floor(Math.random() * chars.length));
		}
		return id;
	};

	self.makeIdByText = function(text) {
		return btoa(text).replace("=", "_");
	};

	self.getRandomInt = function(min, max) {
		min = Math.ceil(min);
		max = Math.floor(max);
		return Math.floor(Math.random() * (max - min + 1)) + min;
	};

	self.formatSize = function(size) {
		if (size > 0) {
			let index = Math.floor( Math.log(size) / Math.log(1024) );
			return (size / Math.pow(1024, index)).toFixed(2) * 1 + " " + ["B", "KiB", "MiB", "GiB", "TiB"][index];
		} else {
			return 0;
		}
	};

	self.formatDuration = function(duration) {
		let millis = parseInt((duration % 1000) / 100);
		let secs = Math.floor((duration / 1000) % 60);
		let mins = Math.floor((duration / (1000 * 60)) % 60);
		let hours = Math.floor((duration / (1000 * 60 * 60)) % 24);
		hours = (hours < 10 ? "0" + hours : hours);
		mins = (mins < 10 ? "0" + mins : mins);
		secs = (secs < 10 ? "0" + secs : secs);
		return `${hours}:${mins}:${secs}.${millis}`;
	};

	self.remap = function(x, a1, b1, a2, b2) {
		let remapped = Math.round((x - a1) / b1 * (b2 - a2) + a2);
		if (remapped < a2) {
			return a2;
		} else if (remapped > b2) {
			return b2;
		}
		return remapped;
	};

	/************************************************************************/

	self.el = new function() {
		return {
			"setOnClick": function(el, callback, prevent_default=true) {
				el.onclick = el.ontouchend = function(event) {
					if (prevent_default) {
						event.preventDefault();
					}
					callback();
				};
			},
			"setOnDown": function(el, callback, prevent_default=true) {
				el.onmousedown = el.ontouchstart = function(event) {
					if (prevent_default) {
						event.preventDefault();
					}
					callback();
				};
			},
			"setOnUp": function(el, callback, prevent_default=true) {
				el.onmouseup = el.ontouchend = function(event) {
					if (prevent_default) {
						event.preventDefault();
					}
					callback();
				};
			},
			"setEnabled": function(el, enabled) {
				if (!enabled && document.activeElement === el) {
					let el_to_focus = (
						el.closest(".modal-window")
						|| el.closest(".window")
						|| el.closest(".menu")
					);
					if (el_to_focus) {
						el_to_focus.focus();
					}
				}
				el.disabled = !enabled;
			},
		};
	};

	self.slider = new function() {
		return {
			"setOnUpDelayed": function(el, delay, execute_callback) {
				el.__execution_timer = null;
				el.__pressed = false;
				el.__postponed = null;

				let clear_timer = function() {
					if (el.__execution_timer) {
						clearTimeout(el.__execution_timer);
						el.__execution_timer = null;
					}
				};

				el.onmousedown = el.ontouchstart = function() {
					clear_timer();
					el.__pressed = true;
				};

				el.onmouseup = el.ontouchend = function(event) {
					let value = self.slider.getValue(el);
					event.preventDefault();
					clear_timer();
					el.__execution_timer = setTimeout(function() {
						el.__pressed = false;
						if (el.__postponed !== null) {
							self.slider.setValue(el, el.__postponed);
							el.__postponed = null;
						}
						execute_callback(value);
					}, delay);
				};
			},
			"setParams": function(el, min, max, step, value, display_callback=null) {
				el.min = min;
				el.max = max;
				el.step = step;
				el.value = value;
				if (display_callback) {
					el.oninput = el.onchange = () => display_callback(self.slider.getValue(el));
					display_callback(self.slider.getValue(el));
					el.__display_callback = display_callback;
				}
			},
			"setRange": function(el, min, max) {
				let value = el.value;
				el.min = min;
				el.max = max;
				if (el.value != value) {
					self.slider.setValue(el, el.value, true);
				}
			},
			"setValue": function(el, value, force=false) {
				if (el.value != value || force) {
					if (el.__pressed) {
						el.__postponed = value;
					} else {
						el.value = value;
						if (el.__display_callback) {
							el.__display_callback(value);
						}
					}
				}
			},
			"getValue": function(el) {
				if (el.step % 1 === 0) {
					return parseInt(el.value);
				} else {
					return parseFloat(el.value);
				}
			},
		};
	};

	self.radio = new function() {
		return {
			"makeItem": function(name, title, value) {
				return `
					<input type="radio" id="${name}-${value}" name="${name}" value="${value}" />
					<label for="${name}-${value}">${title}</label>
				`;
			},
			"setOnClick": function(name, callback, prevent_default=true) {
				for (let el of $$$(`input[type="radio"][name="${name}"]`)) {
					self.el.setOnClick(el, callback, prevent_default);
				}
			},
			"getValue": function(name) {
				return document.querySelector(`input[type="radio"][name="${name}"]:checked`).value;
			},
			"setValue": function(name, value) {
				for (let el of $$$(`input[type="radio"][name="${name}"]`)) {
					el.checked = (el.value === value);
				}
			},
			"clickValue": function(name, value) {
				for (let el of $$$(`input[type="radio"][name="${name}"]`)) {
					if (el.value === value) {
						el.click();
						return;
					}
				}
			},
			"setEnabled": function(name, enabled) {
				for (let el of $$$(`input[type="radio"][name="${name}"]`)) {
					self.el.setEnabled(el, enabled);
				}
			},
		};
	};

	self.selector = new function() {
		return {
			"addOption": function(el, title, value, selected=false) {
				el.add(new Option(title, value, selected, selected));
			},
			"addComment": function(el, title) {
				let option = new Option(title, ".".repeat(30), false, false); // Kinda magic value
				option.disabled = true;
				option.className = "comment";
				el.add(option);
			},
			"addSeparator": function(el) {
				if (!self.browser.is_mobile) {
					self.selector.addComment(el, "\u2500".repeat(30));
				}
			},

			"setValues": function(el, values, empty_title=null) {
				if (values.constructor == Object) {
					values = Object.keys(values).sort();
				}
				let values_json = JSON.stringify(values);
				if (el.__values_json !== values_json) {
					el.options.length = 0;
					for (let value of values) {
						let title = value;
						if (title.length === 0 && empty_title !== null) {
							title = empty_title;
						}
						self.selector.addOption(el, title, value);
					}
					el.__values_json = values_json;
					el.__values = values;
				}
			},
			"setSelectedValue": function(el, value) {
				if (el.__values && el.__values.includes(value)) {
					el.value = value;
				}
			},
		};
	};

	self.progress = new function() {
		return {
			"setValue": function(el, title, percent) {
				el.setAttribute("data-label", title);
				el.querySelector(".progress-value").style.width = `${percent}%`;
			},
			"setPercentOf": function(el, max, value) {
				let percent = Math.round(value * 100 / max);
				self.progress.setValue(el, `${percent}%`, percent);
			},
			"setSizeOf": function(el, title, size, free) {
				let size_str = self.formatSize(size);
				let used = size - free;
				let used_str = self.formatSize(used);
				let percent = used / size * 100;
				title = title.replace("%s", `${used_str} of ${size_str}`);
				self.progress.setValue(el, title, percent);
			},
		};
	};

	self.input = new function() {
		return {
			"getFile": function(el) {
				return (el.files.length ? el.files[0] : null);
			},
		};
	};

	self.hidden = new function() {
		return {
			"setVisible": function(el, visible) {
				el.classList.toggle("hidden", !visible);
			},
			"isVisible": function(el) {
				return !el.classList.contains("hidden");
			},
		};
	};

	self.feature = new function() {
		return {
			"setEnabled": function(el, enabled) {
				el.classList.toggle("feature-disabled", !enabled);
			},
		};
	};

	/************************************************************************/

	let __debug = (new URL(window.location.href)).searchParams.get("debug");

	self.debug = function(...args) {
		if (__debug) {
			__log("DEBUG", ...args);
		}
	};
	self.info = (...args) => __log("INFO", ...args);
	self.error = (...args) => __log("ERROR", ...args);

	let __log = function(label, ...args) {
		let now = (new Date()).toISOString().split("T")[1].replace("Z", "");
		console.log(`[${now}] LOG/${label} --`, ...args);
	};

	/************************************************************************/

	self.is_https = (location.protocol === "https:");

	self.cookies = new function() {
		return {
			"get": function(name) {
				let matches = document.cookie.match(new RegExp(
					"(?:^|; )" + name.replace(/([\.$?*|{}\(\)\[\]\\\/\+^])/g, "\\$1") + "=([^;]*)" // eslint-disable-line no-useless-escape
				));
				return (matches ? decodeURIComponent(matches[1]) : "");
			},
		};
	};

	self.storage = new function() {
		return {
			"get": function(key, default_value) {
				let value = window.localStorage.getItem(key);
				return (value !== null ? value : `${default_value}`);
			},
			"set": (key, value) => window.localStorage.setItem(key, value),

			"getInt": (key, default_value) => parseInt(self.storage.get(key, default_value)),
			"setInt": (key, value) => self.storage.set(key, value),

			"getBool": (key, default_value) => !!parseInt(self.storage.get(key, (default_value ? "1" : "0"))),
			"setBool": (key, value) => self.storage.set(key, (value ? "1" : "0")),

			"bindSimpleSwitch": function(el, key, default_value, callback=null) {
				let value = self.storage.getBool(key, default_value);
				el.checked = value;
				if (callback) {
					callback(value);
				}
				self.el.setOnClick(el, function() {
					if (callback) {
						callback(el.checked);
					}
					self.storage.setBool(key, el.checked);
				}, false);
			},
		};
	};

	self.config = new function() {
		return {
			"get": function(key, default_value) {
				let value = window.getComputedStyle(document.documentElement).getPropertyValue(`--config-ui--${key}`);
				return (value || default_value);
			},
			"getBool": (key, default_value) => !!parseInt(self.config.get(key, (default_value ? "1" : "0"))),
		};
	};

	self.browser = browser;
};

export var $ = (id) => document.getElementById(id);
export var $$ = (cls) => [].slice.call(document.getElementsByClassName(cls));
export var $$$ = (selector) => document.querySelectorAll(selector);
