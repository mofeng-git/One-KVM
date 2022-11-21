/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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


import {tools, $, $$$} from "../tools.js";
import {wm} from "../wm.js";


export function Msd() {
	var self = this;

	/************************************************************************/

	var __state = null;
	var __http = null;

	var __init__ = function() {
		$("msd-led").title = "Unknown state";

		$("msd-image-selector").onchange = __selectImage;
		tools.el.setOnClick($("msd-download-button"), __clickDownloadButton);
		tools.el.setOnClick($("msd-remove-button"), __clickRemoveButton);

		tools.radio.setOnClick("msd-mode-radio", __clickModeRadio);

		tools.el.setOnClick($("msd-rw-switch"), __clickRwSwitch);

		tools.el.setOnClick($("msd-select-new-button"), __toggleSelectSub);
		$("msd-new-file").onchange = __selectNewFile;
		$("msd-new-url").oninput = __selectNewUrl;

		tools.el.setOnClick($("msd-upload-new-button"), __clickUploadNewButton);
		tools.el.setOnClick($("msd-abort-new-button"), __clickAbortNewButton);

		tools.el.setOnClick($("msd-connect-button"), () => __clickConnectButton(true));
		tools.el.setOnClick($("msd-disconnect-button"), () => __clickConnectButton(false));

		tools.el.setOnClick($("msd-reset-button"), __clickResetButton);
	};

	/************************************************************************/

	self.setState = function(state) {
		__state = state;
		__applyState();
	};

	var __selectImage = function() {
		tools.el.setEnabled($("msd-image-selector"), false);
		tools.el.setEnabled($("msd-download-button"), false);
		tools.el.setEnabled($("msd-remove-button"), false);
		__sendParam("image", $("msd-image-selector").value);
	};

	var __clickDownloadButton = function() {
		let name = $("msd-image-selector").value;
		window.open(`/api/msd/read?image=${name}`);
	};

	var __clickRemoveButton = function() {
		let name = $("msd-image-selector").value;
		wm.confirm(`Are you sure you want to remove the image<br><b>${name}</b> from PiKVM?`).then(function(ok) {
			if (ok) {
				let http = tools.makeRequest("POST", `/api/msd/remove?image=${name}`, function() {
					if (http.readyState === 4) {
						if (http.status !== 200) {
							wm.error("Can't remove image:<br>", http.responseText);
						}
					}
				});
			}
		});
	};

	var __clickModeRadio = function() {
		__sendParam("cdrom", tools.radio.getValue("msd-mode-radio"));
	};

	var __clickRwSwitch = function() {
		__sendParam("rw", $("msd-rw-switch").checked);
	};

	var __sendParam = function(name, value) {
		let http = tools.makeRequest("POST", `/api/msd/set_params?${name}=${encodeURIComponent(value)}`, function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					wm.error("Can't configure MSD:<br>", http.responseText);
				}
			}
		});
	};

	var __clickUploadNewButton = function() {
		let file = tools.input.getFile($("msd-new-file"));
		__http = new XMLHttpRequest();
		if (file) {
			__http.open("POST", `/api/msd/write?image=${encodeURIComponent(file.name)}&remove_incomplete=1`, true);
		} else {
			let url = $("msd-new-url").value;
			__http.open("POST", `/api/msd/write_remote?url=${encodeURIComponent(url)}&remove_incomplete=1`, true);
		}
		__http.upload.timeout = 7 * 24 * 3600;
		__http.onreadystatechange = __httpStateChange;
		__http.send(file);
		__applyState();
	};

	var __httpStateChange = function() {
		if (__http.readyState === 4) {
			if (__http.status !== 200) {
				wm.error("Can't upload image to the Mass Storage Drive:<br>", __http.responseText);
			} else if ($("msd-new-url").value.length > 0) {
				let msg = "";
				try {
					let end = __http.responseText.lastIndexOf("\r\n");
					if (end < 0) {
						console.log(1);
						end = __http.responseText.length;
					}
					let begin = __http.responseText.lastIndexOf("\r\n", end - 2);
					if (begin < 0) {
						end = 0;
					}
					let result_str = __http.responseText.slice(begin, end);
					let result = JSON.parse(result_str);
					if (!result.ok) {
						msg = `Can't upload image to the Mass Storage Drive:<br>${result_str}`;
					}
				} catch (err) {
					msg = `Can't parse upload result:<br>${err}`;
				}
				if (msg.length > 0) {
					wm.error(msg);
				}
			}
			tools.hidden.setVisible($("msd-new-sub"), false);
			$("msd-new-file").value = "";
			$("msd-new-url").value = "";
			__http = null;
			__applyState();
		}
	};

	var __clickAbortNewButton = function() {
		__http.onreadystatechange = null;
		__http.abort();
		__http = null;
		tools.progress.setValue($("msd-uploading-progress"), "Aborted", 0);
	};

	var __clickConnectButton = function(connected) {
		let http = tools.makeRequest("POST", `/api/msd/set_connected?connected=${connected}`, function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					wm.error("Switch error:<br>", http.responseText);
				}
			}
			__applyState();
		});
		__applyState();
		tools.el.setEnabled($(`msd-${connected ? "connect" : "disconnect"}-button`), false);
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset Mass Storage Drive?").then(function(ok) {
			if (ok) {
				let http = tools.makeRequest("POST", "/api/msd/reset", function() {
					if (http.readyState === 4) {
						if (http.status !== 200) {
							wm.error("MSD reset error:<br>", http.responseText);
						}
					}
					__applyState();
				});
				__applyState();
			}
		});
	};

	var __toggleSelectSub = function() {
		let el_sub = $("msd-new-sub");
		let visible = tools.hidden.isVisible(el_sub);
		if (visible) {
			$("msd-new-file").value = "";
			$("msd-new-url").value = "";
		}
		tools.hidden.setVisible(el_sub, !visible);
		__applyState();
	};

	var __selectNewFile = function() {
		let el_input = $("msd-new-file");
		let file = tools.input.getFile($("msd-new-file"));
		if (file) {
			$("msd-new-url").value = "";
			if (file.size > __state.storage.size) {
				wm.error("New image is too big for your Mass Storage Drive.<br>Maximum:", tools.formatSize(__state.storage.size));
				el_input.value = "";
			}
		}
		__applyState();
	};

	var __selectNewUrl = function() {
		if ($("msd-new-url").value.length > 0) {
			$("msd-new-file").value = "";
		}
		__applyState();
	};

	var __applyState = function() {
		__applyStateFeatures();
		__applyStateStatus();

		let s = __state;
		let online = (s && s.online);

		if (online) {
			let size_str = tools.formatSize(s.storage.size);
			let used = s.storage.size - s.storage.free;
			let used_str = tools.formatSize(used);
			let percent = used / s.storage.size * 100;
			tools.progress.setValue($("msd-storage-progress"), `Storage: ${used_str} of ${size_str}`, percent);
		} else {
			tools.progress.setValue($("msd-storage-progress"), "Storage: unavailable", 0);
		}

		tools.el.setEnabled($("msd-image-selector"), (online && !s.drive.connected && !s.busy));
		__applyStateImageSelector();
		tools.el.setEnabled($("msd-download-button"), (online && s.drive.image && !s.drive.connected && !s.busy));
		tools.el.setEnabled($("msd-remove-button"), (online && s.drive.image && !s.drive.connected && !s.busy));

		tools.radio.setEnabled("msd-mode-radio", (online && s.features.cdrom && !s.drive.connected && !s.busy));
		tools.radio.setValue("msd-mode-radio", `${Number(online && s.features.cdrom && s.drive.cdrom)}`);

		tools.el.setEnabled($("msd-rw-switch"), (online && s.features.rw && !s.drive.connected && !s.busy));
		$("msd-rw-switch").checked = (online && s.features.rw && s.drive.rw);

		tools.el.setEnabled($("msd-connect-button"), (online && s.drive.image && !s.drive.connected && !s.busy));
		tools.el.setEnabled($("msd-disconnect-button"), (online && s.drive.connected && !s.busy));

		tools.el.setEnabled($("msd-select-new-button"), (online && !s.drive.connected && !__http && !s.busy));
		tools.el.setEnabled($("msd-upload-new-button"),
			(online && !s.drive.connected && (tools.input.getFile($("msd-new-file")) || $("msd-new-url").value.length > 0) && !s.busy));
		tools.el.setEnabled($("msd-abort-new-button"), (online && __http));

		tools.el.setEnabled($("msd-reset-button"), (s && s.enabled && !s.busy));

		tools.el.setEnabled($("msd-new-file"), (online && !s.drive.connected && !__http && !s.busy));
		tools.el.setEnabled($("msd-new-url"), (online && !s.drive.connected && !__http && !s.busy));

		tools.hidden.setVisible($("msd-uploading-sub"), (online && s.storage.uploading));
		$("msd-uploading-name").innerHTML = ((online && s.storage.uploading) ? s.storage.uploading.name : "");
		$("msd-uploading-size").innerHTML = ((online && s.storage.uploading) ? tools.formatSize(s.storage.uploading.size) : "");
		if (online) {
			if (s.storage.uploading) {
				let percent = Math.round(s.storage.uploading.written * 100 / s.storage.uploading.size);
				tools.progress.setValue($("msd-uploading-progress"), `${percent}%`, percent);
			} else if (!__http) {
				tools.progress.setValue($("msd-uploading-progress"), "Waiting for upload (press UPLOAD button) ...", 0);
			}
		} else {
			$("msd-new-file").value = "";
			$("msd-new-url").value = "";
			tools.progress.setValue($("msd-uploading-progress"), "", 0);
		}
	};

	var __applyStateFeatures = function() {
		let s = __state;
		let online = (s && s.online);

		if (s) {
			tools.feature.setEnabled($("msd-dropdown"), s.enabled);
			tools.feature.setEnabled($("msd-reset-button"), s.enabled);
			for (let el of $$$(".msd-cdrom-emulation")) {
				tools.feature.setEnabled(el, s.features.cdrom);
			}
			for (let el of $$$(".msd-rw")) {
				tools.feature.setEnabled(el, s.features.rw);
			}
		}

		tools.hidden.setVisible($("msd-message-offline"), (s && !s.online));
		tools.hidden.setVisible($("msd-message-image-broken"),
			(online && s.drive.image && !s.drive.image.complete && !s.storage.uploading));
		tools.hidden.setVisible($("msd-message-too-big-for-cdrom"),
			(online && s.features.cdrom && s.drive.cdrom && s.drive.image && s.drive.image.size >= 2359296000));
		tools.hidden.setVisible($("msd-message-out-of-storage"),
			(online && s.drive.image && !s.drive.image.in_storage));
		tools.hidden.setVisible($("msd-message-rw-enabled"),
			(online && s.features.rw && s.drive.rw));
		tools.hidden.setVisible($("msd-message-another-user-uploads"),
			(online && s.storage.uploading && !__http));
		tools.hidden.setVisible($("msd-message-downloads"),
			(online && s.storage.downloading));
	};

	var __applyStateStatus = function() {
		let s = __state;
		let online = (s && s.online);

		let led_cls = "led-gray";
		let msg = "Unavailable";

		if (online && s.drive.connected) {
			led_cls = "led-green";
			msg = "Connected to Server";
		} else if (online && s.storage.uploading) {
			led_cls = "led-yellow-rotating-fast";
			msg = "Uploading new image";
		} else if (online && s.storage.downloading) {
			led_cls = "led-yellow-rotating-fast";
			msg = "Serving the image to download";
		} else if (online) { // Sic!
			msg = "Disconnected";
		}

		$("msd-led").className = led_cls;
		$("msd-status").innerHTML = $("msd-led").title = msg;
	};

	var __applyStateImageSelector = function() {
		let s = __state;
		let online = (s && s.online);
		let el = $("msd-image-selector");

		if (!online) {
			el.options.length = 1; // Cleanup
			return;
		}
		if (s.storage.uploading || s.storage.downloading) {
			return;
		}

		if (el.options.length === 0) {
			el.options[0] = new Option("\u2500 Not selected \u2500", "", false, false);
		} else {
			el.options.length = 1;
		}

		let selected_index = 0;
		let index = 1;

		for (let name of Object.keys(s.storage.images).sort()) {
			let image = s.storage.images[name];

			if (!tools.browser.is_mobile) {
				let separator = new Option("\u2500".repeat(30), false, false);
				separator.disabled = true;
				separator.className = "comment";
				el.options[index] = separator;
				++index;
			}

			let option = new Option(name, name, false, false);
			el.options[index] = option;
			if (s.drive.image && s.drive.image.name === name && s.drive.image.in_storage) {
				selected_index = index;
			}
			++index;

			el.options[index] = __makeImageSelectorInfo(image);
			++index;
		}

		if (s.drive.image && !s.drive.image.in_storage) {
			el.options[index] = new Option(s.drive.image.name, "", false, false);
			el.options[index + 1] = __makeImageSelectorInfo(s.drive.image);
			selected_index = el.options.length - 2;
		}

		el.selectedIndex = selected_index;
	};

	var __makeImageSelectorInfo = function(image) {
		let title = `\xA0\xA0\xA0\xA0\xA0\u2570 ${tools.formatSize(image.size)}`;
		title += (image.complete ? "" : ", broken");
		if (image.in_storage !== undefined && !image.in_storage) {
			title += ", out of storage";
		}

		let dt = new Date(image.mod_ts * 1000);
		dt = new Date(dt.getTime() - (dt.getTimezoneOffset() * 60000));
		title += " \u2500 " + dt.toISOString().slice(0, -8).replaceAll("-", ".").replace("T", "-");

		let el = new Option(title, "", false, false);
		el.disabled = true;
		el.className = "comment";
		return el;
	};

	__init__();
}
