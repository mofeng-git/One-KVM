/*****************************************************************************
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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


export var tools = new function() {
	this.setDefault = function(dict, key, value) {
		if (!(key in dict)) {
			dict[key] = value;
		}
	};

	/************************************************************************/

	this.makeRequest = function(method, url, callback, body=null, content_type=null) {
		let http = new XMLHttpRequest();
		http.open(method, url, true);
		if (content_type) {
			http.setRequestHeader("Content-Type", content_type);
		}
		http.onreadystatechange = callback;
		http.timeout = 15000;
		http.send(body);
		return http;
	};

	/************************************************************************/

	this.upperFirst = function(text) {
		return text[0].toUpperCase() + text.slice(1);
	};

	this.makeId = function() {
		let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
		let id = "";
		for (let count = 0; count < 16; ++count) {
			id += chars.charAt(Math.floor(Math.random() * chars.length));
		}
		return id;
	};

	this.formatSize = function(size) {
		if (size > 0) {
			let index = Math.floor( Math.log(size) / Math.log(1024) );
			return (size / Math.pow(1024, index)).toFixed(2) * 1 + " " + ["B", "KiB", "MiB", "GiB", "TiB"][index];
		} else {
			return 0;
		}
	};

	this.formatDuration = function(duration) {
		let millis = parseInt((duration % 1000) / 100);
		let secs = Math.floor((duration / 1000) % 60);
		let mins = Math.floor((duration / (1000 * 60)) % 60);
		let hours = Math.floor((duration / (1000 * 60 * 60)) % 24);
		hours = (hours < 10 ? "0" + hours : hours);
		mins = (mins < 10 ? "0" + mins : mins);
		secs = (secs < 10 ? "0" + secs : secs);
		return `${hours}:${mins}:${secs}.${millis}`;
	};

	/************************************************************************/

	this.getCookie = function(name) {
		let matches = document.cookie.match(new RegExp(
			"(?:^|; )" + name.replace(/([\.$?*|{}\(\)\[\]\\\/\+^])/g, "\\$1") + "=([^;]*)" // eslint-disable-line no-useless-escape
		));
		return (matches ? decodeURIComponent(matches[1]) : "");
	};

	this.setOnClick = function(el, callback, prevent_default=true) {
		el.onclick = el.ontouchend = function(event) {
			if (prevent_default) {
				event.preventDefault();
			}
			callback();
		};
	};
	this.setOnDown = function(el, callback, prevent_default=true) {
		el.onmousedown = el.ontouchstart = function(event) {
			if (prevent_default) {
				event.preventDefault();
			}
			callback();
		};
	};
	this.setOnUp = function(el, callback, prevent_default=true) {
		el.onmouseup = el.ontouchend = function(event) {
			if (prevent_default) {
				event.preventDefault();
			}
			callback();
		};
	};

	this.sliderSetOnUp = function(el, delay, display_callback, execute_callback) {
		el.execution_timer = null;
		el.activated = false;

		let clear_timer = function() {
			if (el.execution_timer) {
				clearTimeout(el.execution_timer);
				el.execution_timer = null;
			}
		};

		el.oninput = el.onchange = () => display_callback(el.value);

		el.onmousedown = el.ontouchstart = function() {
			clear_timer();
			el.activated = true;
		};

		el.onmouseup = el.ontouchend = function(event) {
			let value = el.value;
			event.preventDefault();
			clear_timer();
			el.execution_timer = setTimeout(function() {
				execute_callback(value);
			}, delay);
		};
	};
	this.sliderSetParams = function(el, min, max, step, value) {
		el.min = min;
		el.max = max;
		el.step = step;
		el.value = value;
	};

	this.radioMakeItem = function(name, title, value) {
		return `
			<input type="radio" id="${name}-${value}" name="${name}" value="${value}" />
			<label for="${name}-${value}">${title}</label>
		`;
	};
	this.radioSetOnClick = function(name, callback, prevent_default=true) {
		for (let el of $$$(`input[type="radio"][name="${name}"]`)) {
			this.setOnClick(el, callback, prevent_default);
		}
	};
	this.radioGetValue = function(name) {
		return document.querySelector(`input[type="radio"][name="${name}"]:checked`).value;
	};
	this.radioSetValue = function(name, value) {
		for (let el of $$$(`input[type="radio"][name="${name}"]`)) {
			el.checked = (el.value === value);
		}
	};
	this.radioClickValue = function(name, value) {
		for (let el of $$$(`input[type="radio"][name="${name}"]`)) {
			if (el.value === value) {
				el.click();
				return;
			}
		}
	};
				

	this.progressSetValue = function(el, title, percent) {
		el.setAttribute("data-label", title);
		$(`${el.id}-value`).style.width = `${percent}%`;
	};

	this.hiddenSetVisible = function(el, visible) {
		el.classList.toggle("hidden", !visible);
	};

	this.featureSetEnabled = function(el, enabled) {
		el.classList.toggle("feature-disabled", !enabled);
	};

	/************************************************************************/

	let __debug = (new URL(window.location.href)).searchParams.get("debug");

	this.debug = function(...args) {
		if (__debug) {
			__log("DEBUG", ...args);
		}
	};
	this.info = (...args) => __log("INFO", ...args);
	this.error = (...args) => __log("ERROR", ...args);

	let __log = function(label, ...args) {
		let now = (new Date()).toISOString().split("T")[1].replace("Z", "");
		console.log(`[${now}] LOG/${label} --`, ...args);
	};

	/************************************************************************/

	this.browser = new function() {
		// https://stackoverflow.com/questions/9847580/how-to-detect-safari-chrome-ie-firefox-and-opera-browser/9851769

		// Opera 8.0+
		let is_opera = (
			(!!window.opr && !!opr.addons) // eslint-disable-line no-undef
			|| !!window.opera
			|| (navigator.userAgent.indexOf(" OPR/") >= 0)
		);

		// Firefox 1.0+
		let is_firefox = (typeof InstallTrigger !== "undefined");

		// Safari 3.0+ "[object HTMLElementConstructor]" 
		let is_safari = (/constructor/i.test(window.HTMLElement) || (function (p) {
			return p.toString() === "[object SafariRemoteNotification]";
		})(!window["safari"] || (typeof safari !== "undefined" && safari.pushNotification))); // eslint-disable-line no-undef

		// Chrome 1+
		let is_chrome = !!window.chrome;

		// Blink engine detection
		let is_blink = ((is_chrome || is_opera) && !!window.CSS);

		// iOS browsers
		// https://stackoverflow.com/questions/9038625/detect-if-device-is-ios
		let is_ios = (!!navigator.platform && /iPad|iPhone|iPod/.test(navigator.platform));

		// Any browser on Mac
		let is_mac = ((
			window.navigator.oscpu
			|| window.navigator.platform
			|| window.navigator.appVersion
			|| "Unknown"
		).indexOf("Mac") !== -1);

		return {
			"is_opera": is_opera,
			"is_firefox": is_firefox,
			"is_safari": is_safari,
			"is_chrome": is_chrome,
			"is_blink": is_blink,
			"is_ios": is_ios,
			"is_mac": is_mac,
		};
	};
	this.info("Browser:", this.browser);

	this.https = (location.protocol === "https:");

	this.storage = new function() {
		return {
			"get": function(key, default_value) {
				let value = window.localStorage.getItem(key);
				return (value !== null ? value : default_value);
			},
			"set": (key, value) => window.localStorage.setItem(key, value),
		};
	};
};

export var $ = (id) => document.getElementById(id);
export var $$ = (cls) => [].slice.call(document.getElementsByClassName(cls));
export var $$$ = (selector) => document.querySelectorAll(selector);
