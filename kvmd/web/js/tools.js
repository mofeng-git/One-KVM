var tools = new function() {
	this.makeRequest = function(method, url, callback, timeout=null) {
		var http = new XMLHttpRequest();
		http.open(method, url, true)
		http.onreadystatechange = callback;
		http.timeout = timeout ? timeout : 5000;
		http.send();
		return http;
	};

	this.setButtonBusy = function(el_button, busy) {
		el_button.disabled = busy;
		el_button.style.cursor = (busy ? "wait" : "default");
	};

	var __debug = (new URL(window.location.href)).searchParams.get("debug");

	this.debug = function(...args) {
		if (__debug) {
			console.log(...args);
		}
	};

	this.info = console.log;
	this.error = console.error;
};

var $ = function(id) { return document.getElementById(id); };
