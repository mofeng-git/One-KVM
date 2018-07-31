var tools = new function() {
	this.makeRequest = function(method, url, callback, timeout=null) {
		var http = new XMLHttpRequest();
		http.open(method, url, true)
		http.onreadystatechange = callback;
		http.timeout = timeout ? timeout : 5000;
		http.send();
		return http;
	};

	var __debug = (new URL(window.location.href)).searchParams.get("debug");

	this.debug = function(...args) {
		if (__debug) {
			console.log("LOG/DEBUG", ...args);
		}
	};

	this.info = (...args) => console.log("LOG/INFO", ...args);
	this.error = (...args) => console.error("LOG/ERROR", ...args);
};

var $ = function(id) { return document.getElementById(id); };
