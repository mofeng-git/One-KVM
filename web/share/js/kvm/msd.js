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

		tools.setOnClick($("msd-emulate-cdrom-checkbox"), __clickCdromSwitch);

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

	var __clickCdromSwitch = function() {
		__sendParam("cdrom", ($("msd-emulate-cdrom-checkbox").checked ? "1" : "0"));
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

	var __clickAbortUploadingButton = function() {
		__upload_http.onreadystatechange = null;
		__upload_http.upload.onprogress = null;
		__upload_http.abort();
		__upload_http = null;
		$("msd-uploading-progress").setAttribute("data-label", "Aborted");
		$("msd-uploading-progress-value").style.width = "0%";
	};

	var __clickConnectButton = function(connect) {
		let action = (connect ? "connect" : "disconnect");
		let http = tools.makeRequest("POST", `/api/msd/${action}`, function() {
			if (http.readyState === 4) {
				if (http.status !== 200) {
					wm.error("Switch error:<br>", http.responseText);
				}
			}
			__applyState();
		});
		__applyState();
		wm.switchDisabled($(`msd-${action}-button`), true);
	};

	var __selectNewImageFile = function() {
		let el_input = $("msd-select-new-image-file");
		let image_file = (el_input.files.length ? el_input.files[0] : null);
		if (image_file && image_file.size > __state.storage.size) {
			wm.error("New image is too big for your Mass Storage Device.<br>Maximum:", __formatSize(__state.storage.size));
			el_input.value = "";
			image_file = null;
		}
		__image_file = image_file;
		__applyState();
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset Mass Storage Device?").then(function(ok) {
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
			for (let el of $$$(".msd-single-storage")) {
				el.classList.toggle("msd-feature-disabled", __state.features.multi);
			}
			for (let el of $$$(".msd-multi-storage")) {
				el.classList.toggle("msd-feature-disabled", !__state.features.multi);
			}

			$("msd-dropdown").classList.toggle("feature-disabled", !__state.enabled);
			$("msd-reset-button").classList.toggle("feature-disabled", !__state.enabled);

			if (__state.online && __state.drive.connected) {
				$("msd-another-another-user-uploads").style.display = "none";
				$("msd-led").className = "led-green";
				$("msd-status").innerHTML = $("msd-led").title = "Connected to Server";
			} else if (__state.online && __state.storage.uploading) {
				if (!__upload_http) {
					$("msd-another-another-user-uploads").style.display = "block";
				}
				$("msd-led").className = "led-yellow-rotating-fast";
				$("msd-status").innerHTML = $("msd-led").title = "Uploading new image";
			} else {
				$("msd-another-another-user-uploads").style.display = "none";
				$("msd-led").className = "led-gray";
				if (__state.online) {
					$("msd-status").innerHTML = $("msd-led").title = "Disconnected";
				} else {
					$("msd-status").innerHTML = $("msd-led").title = "Unavailable";
				}
			}

			$("msd-offline").style.display = (__state.online ? "none" : "block");
			$("msd-drive-image-broken").style.display = (
				__state.online && __state.drive.image &&
				!__state.drive.image.complete && !__state.drive.uploading ? "block" : "none"
			);

			$("msd-drive-image-name").innerHTML = (__state.online && __state.drive.image ? __state.drive.image.name : "None");
			$("msd-drive-image-size").innerHTML = (__state.online && __state.drive.image ? __formatSize(__state.drive.image.size) : "None");

			if (__state.online) {
				let size = __state.storage.size;
				let used = __state.storage.size - __state.storage.free;
				$("msd-storage-size").innerHTML = __formatSize(size);
				$("msd-storage-progress").setAttribute("data-label", `Storage: ${__formatSize(used)} of ${__formatSize(size)} used`);
				$("msd-storage-progress-value").style.width = `${used / size * 100}%`;
			} else {
				$("msd-storage-size").innerHTML = "Unavailable";
				$("msd-storage-progress").setAttribute("data-label", "Storage: unavailable");
				$("msd-storage-progress-value").style.width = "0%";
			}

			wm.switchDisabled($("msd-emulate-cdrom-checkbox"), (!__state.online || !__state.features.cdrom, __state.drive.connected || __state.busy));
			if (__state.features.multi) {
				wm.switchDisabled($("msd-connect-button"), (!__state.online || !__state.drive.image || __state.drive.connected || __state.busy));
			} else {
				wm.switchDisabled($("msd-connect-button"), (!__state.online || __state.drive.connected || __state.busy));
			}
			wm.switchDisabled($("msd-disconnect-button"), (!__state.online || !__state.drive.connected || __state.busy));
			wm.switchDisabled($("msd-select-new-image-button"), (!__state.online || __state.drive.connected || __state.busy || __upload_http));
			wm.switchDisabled($("msd-upload-new-image-button"), (!__state.online || __state.drive.connected || __state.busy || !__image_file));
			wm.switchDisabled($("msd-abort-uploading-button"), (!__state.online || !__upload_http));
			wm.switchDisabled($("msd-reset-button"), (!__state.enabled || __state.busy));

			$("msd-emulate-cdrom-checkbox").checked = (__state.online && __state.features.cdrom && __state.drive.cdrom);
			$("msd-new-image").style.display = (__image_file ? "block" : "none");
			$("msd-uploading-progress").setAttribute("data-label", "Waiting for upload ...");
			$("msd-uploading-progress-value").style.width = "0%";
			$("msd-new-image-name").innerHTML = (__image_file ? __image_file.name : "");
			$("msd-new-image-size").innerHTML = (__image_file ? __formatSize(__image_file.size) : "");

		} else {
			$("msd-another-another-user-uploads").style.display = "none";
			$("msd-led").className = "led-gray";
			$("msd-status").innerHTML = "";
			$("msd-led").title = "";
			$("msd-offline").style.display = "none";
			$("msd-drive-image-broken").style.display = "none";
			$("msd-drive-image-name").innerHTML = "";
			$("msd-drive-image-size").innerHTML = "";
			$("msd-storage-size").innerHTML = "";

			wm.switchDisabled($("msd-emulate-cdrom-checkbox"), true);
			wm.switchDisabled($("msd-connect-button"), true);
			wm.switchDisabled($("msd-disconnect-button"), true);
			wm.switchDisabled($("msd-select-new-image-button"), true);
			wm.switchDisabled($("msd-upload-new-image-button"), true);
			wm.switchDisabled($("msd-abort-uploading-button"), true);
			wm.switchDisabled($("msd-reset-button"), true);

			$("msd-emulate-cdrom-checkbox").checked = false;
			$("msd-select-new-image-file").value = "";
			$("msd-new-image").style.display = "none";
			$("msd-uploading-progress").setAttribute("data-label", "");
			$("msd-uploading-progress-value").style.width = "0%";
			$("msd-new-image-name").innerHTML = "";
			$("msd-new-image-size").innerHTML = "";
		}
	};

	var __formatSize = function(size) {
		if (size > 0) {
			let index = Math.floor( Math.log(size) / Math.log(1024) );
			return (size / Math.pow(1024, index)).toFixed(2) * 1 + " " + ["B", "kB", "MB", "GB", "TB"][index];
		} else {
			return 0;
		}
	};

	var __uploadStateChange = function() {
		if (__upload_http.readyState === 4) {
			if (__upload_http.status !== 200) {
				wm.error("Can't upload image to the Mass Storage Device:<br>", __upload_http.responseText);
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
			$("msd-uploading-progress").setAttribute("data-label", percent + "%");
			$("msd-uploading-progress-value").style.width = percent + "%";
		}
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

	__init__();
}
