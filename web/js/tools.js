var tools = new function() {
	var __debug = (new URL(window.location.href)).searchParams.get("debug");

	this.makeRequest = function(method, url, callback, timeout=null) {
		var http = new XMLHttpRequest();
		http.open(method, url, true);
		http.onreadystatechange = callback;
		http.timeout = (timeout ? timeout : 5000);
		http.send();
		return http;
	};

	this.getCookie = function(name) {
		var matches = document.cookie.match(new RegExp(
			"(?:^|; )" + name.replace(/([\.$?*|{}\(\)\[\]\\\/\+^])/g, "\\$1") + "=([^;]*)" // eslint-disable-line no-useless-escape
		));
		return (matches ? decodeURIComponent(matches[1]) : "");
	};

	this.setOnClick = function(el, callback) {
		el.onclick = el.ontouchend = function(event) {
			event.preventDefault();
			callback();
		};
	};
	this.setOnDown = function(el, callback) {
		el.onmousedown = el.ontouchstart = function(event) {
			event.preventDefault();
			callback();
		};
	};
	this.setOnUp = function(el, callback) {
		el.onmouseup = el.ontouchend = function(event) {
			event.preventDefault();
			callback();
		};
	};

	this.setOnUpSlider = function(el, delay, display_callback, execute_callback) {
		el.execution_timer = null;
		el.activated = false;

		var clear_timer = function() {
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
			event.preventDefault();
			clear_timer();
			el.execution_timer = setTimeout(function() {
				execute_callback(el.value);
			}, delay);
		};
	};

	this.debug = function(...args) {
		if (__debug) {
			console.log("LOG/DEBUG", ...args);  // eslint-disable-line no-console
		}
	};
	this.info = (...args) => console.log("LOG/INFO", ...args);  // eslint-disable-line no-console
	this.error = (...args) => console.error("LOG/ERROR", ...args);  // eslint-disable-line no-console

	this.browser = new function() {
		// https://stackoverflow.com/questions/9847580/how-to-detect-safari-chrome-ie-firefox-and-opera-browser/9851769

		// Opera 8.0+
		var is_opera = (
			(!!window.opr && !!opr.addons) // eslint-disable-line no-undef
			|| !!window.opera
			|| (navigator.userAgent.indexOf(" OPR/") >= 0)
		);

		// Firefox 1.0+
		var is_firefox = (typeof InstallTrigger !== "undefined");

		// Safari 3.0+ "[object HTMLElementConstructor]" 
		var is_safari = (/constructor/i.test(window.HTMLElement) || (function (p) {
			return p.toString() === "[object SafariRemoteNotification]";
		})(!window["safari"] || (typeof safari !== "undefined" && safari.pushNotification))); // eslint-disable-line no-undef

		// Chrome 1+
		var is_chrome = (!!window.chrome && !!window.chrome.webstore);

		// Blink engine detection
		var is_blink = ((is_chrome || is_opera) && !!window.CSS);

		// iOS browsers
		// https://stackoverflow.com/questions/9038625/detect-if-device-is-ios
		var is_ios = (!!navigator.platform && /iPad|iPhone|iPod/.test(navigator.platform));

		return {
			"is_opera": is_opera,
			"is_firefox": is_firefox,
			"is_safari": is_safari,
			"is_chrome": is_chrome,
			"is_blink": is_blink,
			"is_ios": is_ios,
		};
	};
	this.info("Browser:", this.browser);
};

var $ = (id) => document.getElementById(id);
var $$ = (cls) => document.getElementsByClassName(cls);
