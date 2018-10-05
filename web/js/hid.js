function Hid() {
	var self = this;

	/********************************************************************************/

	var __ws = null;

	var __chars_to_codes = {};
	var __codes_delay = 50;

	var __keyboard = new Keyboard();
	var __mouse = new Mouse();

	var __init__ = function() {
		var __hidden_attr = null;
		var __visibility_change_attr = null;

		if (typeof document.hidden !== "undefined") {
			__hidden_attr = "hidden";
			__visibility_change_attr = "visibilitychange";
		} else if (typeof document.webkitHidden !== "undefined") {
			__hidden_attr = "webkitHidden";
			__visibility_change_attr = "webkitvisibilitychange";
		} else if (typeof document.mozHidden !== "undefined") {
			__hidden_attr = "mozHidden";
			__visibility_change_attr = "mozvisibilitychange";
		}

		if (__visibility_change_attr) {
			document.addEventListener(
				__visibility_change_attr,
				function() {
					if (document[__hidden_attr]) {
						__releaseAll();
					}
				},
				false
			);
		}

		window.onpagehide = __releaseAll;
		window.onblur = __releaseAll;

		__chars_to_codes = __buildCharsToCodes();
		tools.setOnClick($("pak-button"), __clickPasteAsKeysButton);

		tools.setOnClick($("hid-reset-button"), __clickResetButton);

		Array.prototype.forEach.call(document.querySelectorAll("[data-shortcut]"), function(el_shortcut) {
			tools.setOnClick(el_shortcut, () => __emitShortcut(el_shortcut.getAttribute("data-shortcut").split(" ")));
		});
	};

	/********************************************************************************/

	self.setSocket = function(ws) {
		__ws = ws;
		__keyboard.setSocket(ws);
		__mouse.setSocket(ws);
		$("pak-text").disabled = $("pak-button").disabled = $("hid-reset-button").disabled = !ws;
	};

	var __releaseAll = function() {
		__keyboard.releaseAll();
	};

	var __emitShortcut = function(codes) {
		return new Promise(function(resolve) {
			tools.debug("Emitting keys:", codes);

			var raw_events = [];
			[[codes, true], [codes.slice().reverse(), false]].forEach(function(op) {
				var [op_codes, state] = op;
				op_codes.forEach(function(code) {
					raw_events.push({code: code, state: state});
				});
			});

			var index = 0;
			var iterate = () => setTimeout(function() {
				__keyboard.fireEvent(raw_events[index].code, raw_events[index].state);
				++index;
				if (index < raw_events.length) {
					iterate();
				} else {
					resolve(null);
				}
			}, __codes_delay);
			iterate();
		});
	};

	var __buildCharsToCodes = function() {
		var chars_to_codes = {
			"\n": ["Enter"],
			"\t": ["Tab"],
			" ": ["Space"],
			"`": ["Backquote"],   "~": ["ShiftLeft", "Backquote"],
			"\\": ["Backslash"],  "|": ["ShiftLeft", "Backslash"],
			"[": ["BracketLeft"], "{": ["ShiftLeft", "BracketLeft"],
			"]": ["BracketLeft"], "}": ["ShiftLeft", "BracketRight"],
			",": ["Comma"],       "<": ["ShiftLeft", "Comma"],
			".": ["Period"],      ">": ["ShiftLeft", "Period"],
			"1": ["Digit1"],      "!": ["ShiftLeft", "Digit1"],
			"2": ["Digit2"],      "@": ["ShiftLeft", "Digit2"],
			"3": ["Digit3"],      "#": ["ShiftLeft", "Digit3"],
			"4": ["Digit4"],      "$": ["ShiftLeft", "Digit4"],
			"5": ["Digit5"],      "%": ["ShiftLeft", "Digit5"],
			"6": ["Digit6"],      "^": ["ShiftLeft", "Digit6"],
			"7": ["Digit7"],      "&": ["ShiftLeft", "Digit7"],
			"8": ["Digit8"],      "*": ["ShiftLeft", "Digit8"],
			"9": ["Digit9"],      "(": ["ShiftLeft", "Digit9"],
			"0": ["Digit0"],      ")": ["ShiftLeft", "Digit0"],
			"-": ["Minus"],       "_": ["ShiftLeft", "Minus"],
			"'": ["Quote"],       "\"": ["ShiftLeft", "Quote"],
			";": ["Semicolon"],   ":": ["ShiftLeft", "Semicolon"],
			"/": ["Slash"],       "?": ["ShiftLeft", "Slash"],
			"=": ["Equal"],       "+": ["ShiftLeft", "Equal"],
		};

		for (var ch = "a".charCodeAt(0); ch <= "z".charCodeAt(0); ++ch) {
			var low = String.fromCharCode(ch);
			var up = low.toUpperCase();
			var code = "Key" + up;
			chars_to_codes[low] = [code];
			chars_to_codes[up] = ["ShiftLeft", code];
		}

		return chars_to_codes;
	};

	var __clickPasteAsKeysButton = function() {
		var text = $("pak-text").value.replace(/[^\x00-\x7F]/g, "");  // eslint-disable-line no-control-regex
		if (text) {
			var clipboard_codes = [];
			var codes_count = 0;
			[...text].forEach(function(ch) {
				var codes = __chars_to_codes[ch];
				if (codes) {
					codes_count += codes.length;
					clipboard_codes.push(codes);
				}
			});

			var confirm_msg = (
				"You are going to automatically type " + codes_count
				+ " characters from the system clipboard."
				+ " It will take " + (__codes_delay * codes_count * 2 / 1000) + " seconds.<br>"
				+ "<br>Are you sure you want to continue?<br>"
			);

			ui.confirm(confirm_msg).then(function(ok) {
				if (ok) {
					$("pak-text").disabled = true;
					$("pak-button").disabled = true;
					$("pak-led").className = "led-pak-typing";
					$("pak-led").title = "Autotyping...";

					tools.debug("Paste-as-keys:", text);

					var index = 0;
					var iterate = function() {
						__emitShortcut(clipboard_codes[index]).then(function() {
							++index;
							if (index < clipboard_codes.length && __ws) {
								iterate();
							} else {
								$("pak-text").value = "";
								$("pak-text").disabled = false;
								$("pak-button").disabled = false;
								$("pak-led").className = "led-off";
								$("pak-led").title = "";
							}
						});
					};
					iterate();
				} else {
					$("pak-text").value = "";
				}
			});
		}
	};

	var __clickResetButton = function() {
		var http = tools.makeRequest("POST", "/kvmd/hid/reset", function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					ui.error("HID reset error:<br>", http.responseText);
				}
			}
		});
	};

	__init__();
}
