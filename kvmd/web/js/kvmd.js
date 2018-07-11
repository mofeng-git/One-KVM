KVMD_BASE_URL = "/kvmd"


// -----------------------------------------------------------------------------
function runKvmdSession() {
	var ws = new WebSocket("ws://" + location.host + KVMD_BASE_URL + "/ws");

	ws.onopen = function(event) {
		alert("Session opened and keyboard will be captured");
		__installHidHandlers(ws);
		__setSessionStatus("session-opened", "Session opened (keyboard captured)");
	};

	ws.onmessage = function(event) {
		// console.log("KVMD:", event.data);
		event = JSON.parse(event.data);
		if (event.msg_type == "event") {
			if (event.msg.event == "atx_state") {
				document.getElementById("power-led").className = "power-led-" + (event.msg.event_attrs.leds.power ? "on" : "off");
				document.getElementById("hdd-led").className = "hdd-led-" + (event.msg.event_attrs.leds.hdd ? "on" : "off");
			}
		}
	};

	ws.onclose = function(event) {
		__clearHidHandlers();
		__setSessionStatus("session-closed", "Session closed (keyboard free), trying to reconnect...");
		document.getElementById("power-led").className = "power-led-off";
		document.getElementById("hdd-led").className = "hdd-led-off";
		setTimeout(runKvmdSession, 5000);
	};

	ws.onerror = function(error) {
		ws.close();
	};
}

function __setSessionStatus(cls, msg) {
	var el_session_status = document.getElementById("session-status");
	el_session_status.innerHTML = msg;
	el_session_status.className = cls;
}

function __installHidHandlers(ws) {
	// https://www.codeday.top/2017/05/03/24906.html
	document.onkeydown = (event) => __onKeyEvent(ws, event, true);
	document.onkeyup = (event) => __onKeyEvent(ws, event, false);
}

function __clearHidHandlers() {
	document.onkeydown = null;
	document.onkeyup = null;
}

function __onKeyEvent(ws, event, state) {
    if (!event.metaKey) { // https://github.com/wesbos/keycodes/blob/gh-pages/scripts.js
        event.preventDefault();
    }
    // console.log("KVMD: Key", (state ? "pressed:" : "released:"), event)
    ws.send(JSON.stringify({
        event_type: "key",
        key: event.code,
        state: state,
    }));
}


// -----------------------------------------------------------------------------
function clickPowerButton() {
	if (confirm("Are you sure to click the power button?")) {
		__clickButton("power");
	}
}

function clickPowerButtonLong() {
	if (confirm("Are you sure to perform the long press of the power button?")) {
		__clickButton("power_long");
	}
}

function clickResetButton() {
	if (confirm("Are you sure to reboot the server?")) {
		__clickButton("reset");
	}
}

function __clickButton(button) {
	var http = new XMLHttpRequest();
	http.open("POST", KVMD_BASE_URL + "/atx/click?button=" + button, true);
	http.onreadystatechange = function() {
		if (http.readyState == 4) {
			if (http.status == 200) {
				alert("Clicked!")
			} else {
				alert("Click error: " + http.responseText);
			}
		}
	}
	http.send();
}
