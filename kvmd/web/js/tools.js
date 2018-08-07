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

	this.debug = function(...args) {
		if (__debug) {
			console.log("LOG/DEBUG", ...args);  // eslint-disable-line no-console
		}
	};

	this.info = (...args) => console.log("LOG/INFO", ...args);  // eslint-disable-line no-console
	this.error = (...args) => console.error("LOG/ERROR", ...args);  // eslint-disable-line no-console
};

var $ = (id) => document.getElementById(id);
