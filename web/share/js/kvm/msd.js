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


import {tools, $, $$$} from "../tools.js";
import {wm} from "../wm.js";


export function Msd() {
	var self = this;

	/************************************************************************/

	var __state = null;
	var __upload_http = null;
	var __image_file = null;

	var __init__ = function() {
		$("msd-led").title = "Unknown state";

		$("msd-image-selector").onchange = __selectImage;
		tools.setOnClick($("msd-remove-image"), __clickRemoveImageButton);

		tools.radioSetOnClick("msd-mode-radio", __clickModeRadio);

		$("msd-select-new-image-file").onchange = __selectNewImageFile;
		tools.setOnClick($("msd-select-new-image-button"), () => $("msd-select-new-image-file").click());

		tools.setOnClick($("msd-upload-new-image-button"), __clickUploadNewImageButton);
		tools.setOnClick($("msd-abort-uploading-button"), __clickAbortUploadingButton);

		tools.setOnClick($("msd-connect-button"), () => __clickConnectButton(true));
		tools.setOnClick($("msd-disconnect-button"), () => __clickConnectButton(false));

		tools.setOnClick($("msd-reset-button"), __clickResetButton);
	};

	/************************************************************************/

	self.setState = function(state) {
		__state = state;
		__applyState();
	};

	var __selectImage = function() {
		wm.setElementEnabled($("msd-image-selector"), false);
		wm.setElementEnabled($("msd-remove-image"), false);
		__sendParam("image", $("msd-image-selector").value);
	};

	var __clickRemoveImageButton = function() {
		let name = $("msd-image-selector").value;
		wm.confirm(`Are you sure you want to remove the image<br><b>${name}</b> from Pi-KVM?`).then(function(ok) {
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
		__sendParam("cdrom", tools.radioGetValue("msd-mode-radio"));
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

	var __clickUploadNewImageButton = function() {
		__upload_http = new XMLHttpRequest();
		__upload_http.open("POST", `/api/msd/write?image=${encodeURIComponent(__image_file.name)}`, true);
		__upload_http.upload.timeout = 15000;
		__upload_http.onreadystatechange = __uploadStateChange;
		__upload_http.send(__image_file);
	};

	var __uploadStateChange = function() {
		if (__upload_http.readyState === 4) {
			if (__upload_http.status !== 200) {
				wm.error("Can't upload image to the Mass Storage Drive:<br>", __upload_http.responseText);
			}
			$("msd-select-new-image-file").value = "";
			__image_file = null;
			__upload_http = null;
			__applyState();
		}
	};

	var __clickAbortUploadingButton = function() {
		__upload_http.onreadystatechange = null;
		__upload_http.abort();
		__upload_http = null;
		tools.progressSetValue($("msd-uploading-progress"), "Aborted", 0);
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
		wm.setElementEnabled($(`msd-${connected ? "connect" : "disconnect"}-button`), false);
	};

	var __selectNewImageFile = function() {
		let el_input = $("msd-select-new-image-file");
		let image_file = (el_input.files.length ? el_input.files[0] : null);
		if (image_file && image_file.size > __state.storage.size) {
			wm.error("New image is too big for your Mass Storage Drive.<br>Maximum:", tools.formatSize(__state.storage.size));
			el_input.value = "";
			image_file = null;
		}
		__image_file = image_file;
		__applyState();
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

	var __applyState = function() {
		__applyStateFeatures();
		__applyStateStatus();

		let s = __state;
		let online = (s && s.online);

		$("msd-image-name").innerHTML = ((online && s.drive.image) ? s.drive.image.name : "None");
		$("msd-image-size").innerHTML = ((online && s.drive.image) ? tools.formatSize(s.drive.image.size) : "None");
		if (online) {
			let size_str = tools.formatSize(s.storage.size);
			let used = s.storage.size - s.storage.free;
			let used_str = tools.formatSize(used);
			$("msd-storage-size").innerHTML = size_str;
			tools.progressSetValue($("msd-storage-progress"), `Storage: ${used_str} of ${size_str}`, used / s.storage.size * 100);
		} else {
			$("msd-storage-size").innerHTML = "Unavailable";
			tools.progressSetValue($("msd-storage-progress"), "Storage: unavailable", 0);
		}

		wm.setElementEnabled($("msd-image-selector"), (online && s.features.multi && !s.drive.connected && !s.busy));
		__applyStateImageSelector();
		wm.setElementEnabled($("msd-remove-image"), (online && s.features.multi && s.drive.image && !s.drive.connected && !s.busy));

		wm.setRadioEnabled("msd-mode-radio", (online && s.features.cdrom && !s.drive.connected && !s.busy));
		tools.radioSetValue("msd-mode-radio", `${Number(online && s.features.cdrom && s.drive.cdrom)}`);

		wm.setElementEnabled($("msd-connect-button"), (online && (!s.features.multi || s.drive.image) && !s.drive.connected && !s.busy));
		wm.setElementEnabled($("msd-disconnect-button"), (online && s.drive.connected && !s.busy));

		wm.setElementEnabled($("msd-select-new-image-button"), (online && !s.drive.connected && !__upload_http && !s.busy));
		wm.setElementEnabled($("msd-upload-new-image-button"), (online && !s.drive.connected && __image_file && !s.busy));
		wm.setElementEnabled($("msd-abort-uploading-button"), (online && __upload_http));

		wm.setElementEnabled($("msd-reset-button"), (s && s.enabled && !s.busy));

		let uploading = (online ? (s.storage.uploading || __image_file) : null);
		tools.hiddenSetVisible($("msd-submenu-new-image"), uploading);
		$("msd-new-image-name").innerHTML = (uploading ? uploading.name : "");
		$("msd-new-image-size").innerHTML = (uploading ? tools.formatSize(uploading.size) : "");
		if (online) {
			if (s.storage.uploading) {
				let percent = Math.round(s.storage.uploading.written * 100 / s.storage.uploading.size);
				tools.progressSetValue($("msd-uploading-progress"), `${percent}%`, percent);
			} else if (!__upload_http) {
				tools.progressSetValue($("msd-uploading-progress"), "Waiting for upload (press UPLOAD button) ...", 0);
			}
		} else {
			$("msd-select-new-image-file").value = "";
			tools.progressSetValue($("msd-uploading-progress"), "", 0);
		}
	};

	var __applyStateFeatures = function() {
		let s = __state;
		let online = (s && s.online);

		if (s) {
			tools.featureSetEnabled($("msd-dropdown"), s.enabled);
			tools.featureSetEnabled($("msd-reset-button"), s.enabled);
			for (let el of $$$(".msd-single-storage")) {
				tools.featureSetEnabled(el, !s.features.multi);
			}
			for (let el of $$$(".msd-multi-storage")) {
				tools.featureSetEnabled(el, s.features.multi);
			}
			for (let el of $$$(".msd-cdrom-emulation")) {
				tools.featureSetEnabled(el, s.features.cdrom);
			}
		}

		tools.hiddenSetVisible($("msd-message-offline"), (s && !s.online));
		tools.hiddenSetVisible($("msd-message-image-broken"),
			(online && s.drive.image && !s.drive.image.complete && !s.storage.uploading));
		tools.hiddenSetVisible($("msd-message-too-big-for-cdrom"),
			(online && s.features.cdrom && s.drive.cdrom && s.drive.image && s.drive.image.size >= 2359296000));
		tools.hiddenSetVisible($("msd-message-out-of-storage"),
			(online && s.features.multi && s.drive.image && !s.drive.image.in_storage));
		tools.hiddenSetVisible($("msd-message-another-user-uploads"),
			(online && s.storage.uploading && !__upload_http));
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
		if (!s.features.multi || s.storage.uploading) {
			return;
		}

		if (el.options.length === 0) {
			el.options[0] = new Option("~ Not selected ~", "", false, false);
		} else {
			el.options.length = 1;
		}

		let precom = "\xA0\xA0\xA0\xA0\xA0\u21b3";
		let selected_index = 0;
		let index = 1;

		for (let name of Object.keys(s.storage.images).sort()) {
			let image = s.storage.images[name];

			let separator = new Option("\u2500".repeat(30), false, false);
			separator.disabled = true;
			separator.className = "comment";
			el.options[index] = separator;
			++index;

			let option = new Option(name, name, false, false);
			el.options[index] = option;
			if (s.drive.image && s.drive.image.name === name && s.drive.image.in_storage) {
				selected_index = index;
			}
			++index;

			let comment = new Option(`${precom} ${tools.formatSize(image.size)}${image.complete ? "" : ", broken"}`, "", false, false);
			comment.disabled = true;
			comment.className = "comment";
			el.options[index] = comment;
			++index;
		}

		if (s.drive.image && !s.drive.image.in_storage) {
			el.options[index] = new Option(s.drive.image.name, "", false, false);
			el.options[index + 1] = new Option(`${precom} ${tools.formatSize(s.drive.image.size)}, out of storage`, "", false, false);
			selected_index = el.options.length - 2;
		}

		el.selectedIndex = selected_index;
	};

	__init__();
}
