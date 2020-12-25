/*****************************************************************************
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
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
		wm.confirm(`Are you sure you want to remove<br>the image <b>${name}</b>?`).then(function(ok) {
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
		let http = tools.makeRequest("POST", `/api/msd/set_params?${name}=${value}`, function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					wm.error("Can't configure MSD:<br>", http.responseText);
				}
			}
		});
	};

	var __clickUploadNewImageButton = function() {
		let form_data = new FormData();
		form_data.append("image", __image_file.name);
		form_data.append("data", __image_file);

		__upload_http = new XMLHttpRequest();
		__upload_http.open("POST", "/api/msd/write", true);
		__upload_http.upload.timeout = 15000;
		__upload_http.onreadystatechange = __uploadStateChange;
		__upload_http.upload.onprogress = __uploadProgress;
		__upload_http.send(form_data);
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

	var __uploadProgress = function(event) {
		if(event.lengthComputable) {
			let percent = Math.round((event.loaded * 100) / event.total);
			tools.progressSetValue($("msd-uploading-progress"), `${percent}%`, percent);
		}
	};

	var __clickAbortUploadingButton = function() {
		__upload_http.onreadystatechange = null;
		__upload_http.upload.onprogress = null;
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
		if (__state) {
			__toggleMsdFeatures();
			tools.featureSetEnabled($("msd-dropdown"), __state.enabled);
			tools.featureSetEnabled($("msd-reset-button"), __state.enabled);

			__showMessageOffline(!__state.online);
			__showMessageImageBroken(__state.online && __state.drive.image && !__state.drive.image.complete && !__state.storage.uploading);
			if (__state.features.cdrom) {
				__showMessageTooBigForCdrom(__state.online && __state.drive.image && __state.drive.cdrom && __state.drive.image.size >= 2359296000);
			}
			__showMessageOutOfStorage(__state.online && __state.features.multi && __state.drive.image && !__state.drive.image.in_storage);

			if (__state.online && __state.drive.connected) {
				__showMessageAnotherUserUploads(false);
				__setStatus("led-green", "Connected to Server");
			} else if (__state.online && __state.storage.uploading) {
				if (!__upload_http) {
					__showMessageAnotherUserUploads(true);
				}
				__setStatus("led-yellow-rotating-fast", "Uploading new image");
			} else {
				__showMessageAnotherUserUploads(false);
				__setStatus("led-gray", (__state.online ? "Disconnected" : "Unavailable"));
			}

			$("msd-image-name").innerHTML = (__state.online && __state.drive.image ? __state.drive.image.name : "None");
			$("msd-image-size").innerHTML = (__state.online && __state.drive.image ? tools.formatSize(__state.drive.image.size) : "None");
			if (__state.online) {
				let size = __state.storage.size;
				let used = __state.storage.size - __state.storage.free;
				$("msd-storage-size").innerHTML = tools.formatSize(size);
				tools.progressSetValue($("msd-storage-progress"), `Storage: ${tools.formatSize(used)} of ${tools.formatSize(size)} used`, used / size * 100);
			} else {
				$("msd-storage-size").innerHTML = "Unavailable";
				tools.progressSetValue($("msd-storage-progress"), "Storage: unavailable", 0);
			}

			wm.setElementEnabled($("msd-image-selector"), (__state.online && __state.features.multi && !__state.drive.connected && !__state.busy));
			if (__state.features.multi && !__state.storage.uploading) {
				__refreshImageSelector();
			}
			wm.setElementEnabled($("msd-remove-image"), (__state.online && __state.features.multi && __state.drive.image && !__state.drive.connected && !__state.busy));

			wm.setRadioEnabled("msd-mode-radio", (__state.online && __state.features.cdrom && !__state.drive.connected && !__state.busy));
			tools.radioSetValue("msd-mode-radio", `${Number(__state.online && __state.features.cdrom && __state.drive.cdrom)}`);

			wm.setElementEnabled($("msd-connect-button"), (__state.online && (!__state.features.multi || __state.drive.image) && !__state.drive.connected && !__state.busy));
			wm.setElementEnabled($("msd-disconnect-button"), (__state.online && __state.drive.connected && !__state.busy));

			wm.setElementEnabled($("msd-select-new-image-button"), (__state.online && !__state.drive.connected && !__upload_http && !__state.busy));
			wm.setElementEnabled($("msd-upload-new-image-button"), (__state.online && !__state.drive.connected && __image_file && !__state.busy));
			wm.setElementEnabled($("msd-abort-uploading-button"), (__state.online && __upload_http));

			wm.setElementEnabled($("msd-reset-button"), (__state.enabled && !__state.busy));

			tools.hiddenSetVisible($("msd-submenu-new-image"), __image_file);
			$("msd-new-image-name").innerHTML = (__image_file ? __image_file.name : "");
			$("msd-new-image-size").innerHTML = (__image_file ? tools.formatSize(__image_file.size) : "");
			if (!__upload_http) {
				tools.progressSetValue($("msd-uploading-progress"), "Waiting for upload ...", 0);
			}

		} else {
			__showMessageOffline(false);
			__showMessageImageBroken(false);
			__showMessageTooBigForCdrom(false);
			__showMessageAnotherUserUploads(false);
			__showMessageOutOfStorage(false);

			__setStatus("led-gray", "");

			$("msd-image-name").innerHTML = "";
			$("msd-image-size").innerHTML = "";
			$("msd-storage-size").innerHTML = "";
			tools.progressSetValue($("msd-storage-progress"), "", 0);

			wm.setElementEnabled($("msd-image-selector"), false);
			$("msd-image-selector").options.length = 1;
			wm.setElementEnabled($("msd-remove-image"), false);

			wm.setRadioEnabled("msd-mode-radio", false);
			tools.radioSetValue("msd-mode-radio", "0");

			wm.setElementEnabled($("msd-connect-button"), false);
			wm.setElementEnabled($("msd-disconnect-button"), false);

			wm.setElementEnabled($("msd-select-new-image-button"), false);
			wm.setElementEnabled($("msd-upload-new-image-button"), false);
			wm.setElementEnabled($("msd-abort-uploading-button"), false);

			wm.setElementEnabled($("msd-reset-button"), false);

			$("msd-select-new-image-file").value = "";
			tools.hiddenSetVisible($("msd-submenu-new-image"), false);
			$("msd-new-image-name").innerHTML = "";
			$("msd-new-image-size").innerHTML = "";
			tools.progressSetValue($("msd-uploading-progress"), "", 0);
		}
	};

	var __toggleMsdFeatures = function() {
		for (let el of $$$(".msd-single-storage")) {
			tools.featureSetEnabled(el, !__state.features.multi);
		}
		for (let el of $$$(".msd-multi-storage")) {
			tools.featureSetEnabled(el, __state.features.multi);
		}
		for (let el of $$$(".msd-cdrom-emulation")) {
			tools.featureSetEnabled(el, __state.features.cdrom);
		}
	};

	var __showMessageOffline = function(visible) {
		tools.hiddenSetVisible($("msd-message-offline"), visible);
	};

	var __showMessageImageBroken = function(visible) {
		tools.hiddenSetVisible($("msd-message-image-broken"), visible);
	};

	var __showMessageTooBigForCdrom = function(visible) {
		tools.hiddenSetVisible($("msd-message-too-big-for-cdrom"), visible);
	};

	var __showMessageOutOfStorage = function(visible) {
		tools.hiddenSetVisible($("msd-message-out-of-storage"), visible);
	};

	var __showMessageAnotherUserUploads = function(visible) {
		tools.hiddenSetVisible($("msd-message-another-user-uploads"), visible);
	};

	var __setStatus = function(led_cls, msg) {
		$("msd-led").className = led_cls;
		$("msd-status").innerHTML = $("msd-led").title = msg;
	};

	var __refreshImageSelector = function() {
		let el = $("msd-image-selector");

		if (el.options.length === 0) {
			el.options[0] = new Option("< Not selected >", "", false, false);
		} else {
			el.options.length = 1; // Cleanup
		}

		if (__state.online) {
			let precom = "\xA0\xA0\xA0\xA0\xA0\u21b3";
			let select_index = 0;
			let index = 1;

			for (let name of Object.keys(__state.storage.images).sort()) {
				let image = __state.storage.images[name];

				let separator = new Option("\u2500".repeat(30), false, false);
				separator.disabled = true;
				separator.className = "comment";
				el.options[index] = separator;
				++index;

				let option = new Option(name, name, false, false);
				el.options[index] = option;
				if (__state.drive.image && __state.drive.image.name === name && __state.drive.image.in_storage) {
					select_index = index;
				}
				++index;

				let comment = new Option(`${precom} ${tools.formatSize(image.size)}${image.complete ? "" : ", broken"}`, "", false, false);
				comment.disabled = true;
				comment.className = "comment";
				el.options[index] = comment;
				++index;
			}

			if (__state.drive.image && !__state.drive.image.in_storage) {
				el.options[index] = new Option(__state.drive.image.name, "", false, false);
				el.options[index + 1] = new Option(`${precom} ${tools.formatSize(__state.drive.image.size)}, out of storage`, "", false, false);
				select_index = el.options.length - 2;
			}

			el.selectedIndex = select_index;
		}
	};

	__init__();
}
