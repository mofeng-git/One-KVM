/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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


import {ROOT_PREFIX} from "../vars.js";
import {tools, $} from "../tools.js";
import {wm} from "../wm.js";


export function Msd() {
	var self = this;

	/************************************************************************/

	var __state = null;
	var __http = null;

	var __init__ = function() {
		$("msd-led").title = "Unknown state";

		tools.selector.addOption($("msd-image-selector"), "\u2500 Not selected \u2500", "");
		$("msd-image-selector").onchange = __selectImage;

		tools.el.setOnClick($("msd-download-button"), __clickDownloadButton);
		tools.el.setOnClick($("msd-remove-button"), __clickRemoveButton);

		tools.radio.setOnClick("msd-mode-radio", () => __sendParam("cdrom", tools.radio.getValue("msd-mode-radio")));
		tools.el.setOnClick($("msd-rw-switch"), () => __sendParam("rw", $("msd-rw-switch").checked));

		tools.el.setOnClick($("msd-select-new-button"), __toggleSelectSub);
		$("msd-new-file").onchange = __selectNewFile;
		$("msd-new-url").oninput = __selectNewUrl;
		$("msd-new-part-selector").onchange = __selectNewFile;

		tools.el.setOnClick($("msd-upload-new-button"), __clickUploadNewButton);
		tools.el.setOnClick($("msd-abort-new-button"), __clickAbortNewButton);

		tools.el.setOnClick($("msd-connect-button"), () => __clickConnectButton(true));
		tools.el.setOnClick($("msd-disconnect-button"), () => __clickConnectButton(false));

		tools.el.setOnClick($("msd-reset-button"), __clickResetButton);
	};

	/************************************************************************/

	self.setState = function(state) {
		if (state) {
			if (!__state) {
				__state = {"storage": {}};
			}
			if (state.enabled !== undefined) {
				__state.enabled = state.enabled;
				tools.feature.setEnabled($("msd-dropdown"), __state.enabled);
			}
			if (__state.enabled !== undefined) {
				if (state.online !== undefined) {
					__state.online = state.online;
				}
				if (state.busy !== undefined) {
					__state.busy = state.busy;
				}
				if (state.drive) { // Null on offline, ignore
					__state.drive = state.drive;
				}
				if (state.storage) { // Null on offline, ignore
					if (state.storage.parts !== undefined) {
						__state.storage.parts = state.storage.parts;
						__updateParts(__state.storage.parts);
					}
					if (state.storage.uploading !== undefined) {
						__state.storage.uploading = state.storage.uploading;
						__updateUploading(__state.storage.uploading);
					}
					if (state.storage.downloading !== undefined) {
						__state.storage.downloading = state.storage.downloading;
					}
					if (state.storage.images !== undefined) {
						__state.storage.images = state.storage.images;
					}
				}
				if (state.drive || (state.storage && state.storage.images !== undefined)) {
					__updateImageSelector(__state.drive, __state.storage.images);
				}
			}
		} else {
			__state = null;
		}
		__refreshControls();
	};

	var __refreshControls = function() {
		__updateControls(__state && (__state.online !== undefined) ? __state : null);
	};

	var __updateControls = function(state) {
		let o = (state && state.online);
		let d = (state ? state.drive : null);
		let s = (state ? state.storage : null);
		let busy = !!(state && state.busy);

		tools.hidden.setVisible($("msd-message-offline"), (state && !state.online));
		tools.hidden.setVisible($("msd-message-image-broken"), (o && d.image && !d.image.complete && !s.uploading));
		tools.hidden.setVisible($("msd-message-too-big-for-dvd"), (o && d.cdrom && d.image && d.image.size >= 33957083136));
		tools.hidden.setVisible($("msd-message-out-of-storage"), (o && d.image && !d.image.in_storage));
		tools.hidden.setVisible($("msd-message-rw-enabled"), (o && d.rw));
		tools.hidden.setVisible($("msd-message-another-user-uploads"), (o && s.uploading && !__http));
		tools.hidden.setVisible($("msd-message-downloads"), (o && s.downloading));

		tools.el.setEnabled($("msd-image-selector"), (o && !d.connected && !busy));
		tools.el.setEnabled($("msd-download-button"), (o && d.image && !d.connected && !busy));
		tools.el.setEnabled($("msd-remove-button"), (o && d.image && d.image.removable && !d.connected && !busy));

		tools.radio.setEnabled("msd-mode-radio", (o && !d.connected && !busy));
		tools.radio.setValue("msd-mode-radio", `${Number(o && d.cdrom)}`);

		tools.el.setEnabled($("msd-rw-switch"), (o && !d.connected && !busy));
		$("msd-rw-switch").checked = (o && d.rw);

		tools.el.setEnabled($("msd-connect-button"), (o && d.image && !d.connected && !busy));
		tools.el.setEnabled($("msd-disconnect-button"), (o && d.connected && !busy));

		tools.el.setEnabled($("msd-select-new-button"), (o && !d.connected && !__http && !busy));
		tools.el.setEnabled($("msd-upload-new-button"),
			(o && !d.connected && (tools.input.getFile($("msd-new-file")) || $("msd-new-url").value.length > 0) && !busy));
		tools.el.setEnabled($("msd-abort-new-button"), (o && __http));

		tools.el.setEnabled($("msd-reset-button"), (state && state.enabled && !busy));

		tools.el.setEnabled($("msd-new-file"), (o && !d.connected && !__http && !busy));
		tools.el.setEnabled($("msd-new-url"), (o && !d.connected && !__http && !busy));
		tools.el.setEnabled($("msd-new-part-selector"), (o && !d.connected && !__http && !busy));

		if (o && s.uploading) {
			tools.hidden.setVisible($("msd-new-sub"), false);
			$("msd-new-file").value = "";
			$("msd-new-url").value = "";
		}
		tools.hidden.setVisible($("msd-uploading-sub"), (o && s.uploading));
		tools.hidden.setVisible($("msd-new-tips"), (o && s.uploading && __http));

		let led_cls = "led-gray";
		let msg = "Unavailable";
		if (o && d.connected) {
			led_cls = "led-green";
			msg = "Connected to Server";
		} else if (o && s.uploading) {
			led_cls = "led-yellow-rotating-fast";
			msg = "Uploading new image";
		} else if (o && s.downloading) {
			led_cls = "led-yellow-rotating-fast";
			msg = "Serving the image to download";
		} else if (o) { // Sic!
			msg = "Disconnected";
		}
		$("msd-led").className = led_cls;
		$("msd-status").innerText = $("msd-led").title = msg;
	};

	var __updateUploading = function(uploading) {
		$("msd-uploading-name").innerText = (uploading ? uploading.name : "");
		$("msd-uploading-size").innerText = (uploading ? tools.formatSize(uploading.size) : "");
		if (uploading) {
			tools.progress.setPercentOf($("msd-uploading-progress"), uploading.size, uploading.written);
		}
	};

	var __updateParts = function(parts) {
		let names = Object.keys(parts).sort();
		{
			let writable = names.filter(name => (name === "" || parts[name].writable));
			let writable_json = JSON.stringify(writable);
			let el = $("msd-new-part-selector");
			if (el.__writable_json !== writable_json) {
				let sel = (el.value || "");
				el.options.length = 0;
				for (let name of writable) {
					let title = (name || "\u2500 Internal \u2500");
					tools.selector.addOption(el, title, name, (name === sel));
				}
				tools.hidden.setVisible($("msd-new-part"), (writable.length > 1));
				el.__writable_json = writable_json;
			}
		}
		{
			let names_json = JSON.stringify(names);
			let el = $("msd-storages");
			if (el.__names_json !== names_json) {
				el.innerHTML = names.map(name => `
					<div class="text">
						<div id="__msd-storage-${tools.makeTextId(name)}-progress" class="progress">
							<span class="progress-value"></span>
						</div>
					</div>
				`).join("<hr>");
				el.__names_json = names_json;
			}
		}
		for (let name of names) {
			let part = parts[name];
			let title = (
				name === ""
				? `${names.length === 1 ? "Storage: %s" : "Internal storage: %s"}` // eslint-disable-line
				: `Storage [${name}${part.writable ? "]" : ", read-only]"}: %s` // eslint-disable-line
			);
			let id = `__msd-storage-${tools.makeTextId(name)}-progress`;
			tools.progress.setSizeOf($(id), title, part.size, part.free);
		}
	};

	var __updateImageSelector = function(drive, images) {
		let sel = "";
		let el = $("msd-image-selector");
		el.options.length = 1;
		for (let name of Object.keys(images).sort()) {
			tools.selector.addSeparator(el);
			tools.selector.addOption(el, name, name);
			tools.selector.addComment(el, __makeImageSelectorInfo(images[name]));
			if (drive.image && drive.image.name === name && drive.image.in_storage) {
				sel = name;
			}
		}
		if (drive.image && !drive.image.in_storage) {
			sel = ".__external__"; // Just some magic name
			tools.selector.addOption(el, drive.image.name, sel);
			tools.selector.addComment(el, __makeImageSelectorInfo(drive.image));
		}
		el.value = sel;
	};

	var __makeImageSelectorInfo = function(image) {
		let text = `\xA0\xA0\xA0\xA0\xA0\u2570 ${tools.formatSize(image.size)}`;
		if (!image.complete) {
			text += ", broken";
		}
		if (image.in_storage !== undefined && !image.in_storage) {
			text += ", out of storage";
		}
		let ts = new Date(image.mod_ts * 1000);
		ts = new Date(ts.getTime() - (ts.getTimezoneOffset() * 60000));
		ts = ts.toISOString().slice(0, -8).replaceAll("-", ".").replace("T", "-");
		return `${text} \u2500 ${ts}`;
	};

	var __selectImage = function() {
		tools.el.setEnabled($("msd-image-selector"), false);
		tools.el.setEnabled($("msd-download-button"), false);
		tools.el.setEnabled($("msd-remove-button"), false);
		__sendParam("image", $("msd-image-selector").value);
	};

	var __clickDownloadButton = function() {
		let e_image = encodeURIComponent($("msd-image-selector").value);
		tools.windowOpen(`api/msd/read?image=${e_image}`);
	};

	var __clickRemoveButton = function() {
		let name = $("msd-image-selector").value;
		wm.confirm("Are you sure you want to remove this image?", name).then(function(ok) {
			if (ok) {
				tools.httpPost("api/msd/remove", {"image": name}, function(http) {
					if (http.status !== 200) {
						wm.error("Can't remove image", http.responseText);
					}
				});
			}
		});
	};

	var __sendParam = function(name, value) {
		tools.httpPost("api/msd/set_params", {[name]: value}, function(http) {
			if (http.status !== 200) {
				wm.error("Can't configure Mass Storage", http.responseText);
			}
			__refreshControls();
		});
	};

	var __clickUploadNewButton = function() {
		let file = tools.input.getFile($("msd-new-file"));
		__http = new XMLHttpRequest();
		let e_prefix = encodeURIComponent($("msd-new-part-selector").value);
		if (file) {
			let e_image = encodeURIComponent(file.name);
			__http.open("POST", `${ROOT_PREFIX}api/msd/write?prefix=${e_prefix}&image=${e_image}&remove_incomplete=1`, true);
		} else {
			let e_url = encodeURIComponent($("msd-new-url").value);
			__http.open("POST", `${ROOT_PREFIX}api/msd/write_remote?prefix=${e_prefix}&url=${e_url}&remove_incomplete=1`, true);
		}
		__http.upload.timeout = 7 * 24 * 3600;
		__http.onreadystatechange = __uploadStateChange;
		__http.send(file);
		__refreshControls();
	};

	var __uploadStateChange = function() {
		if (__http.readyState !== 4) {
			return;
		}
		if (__http.status !== 200) {
			wm.error("Can't upload image", __http.responseText);
		} else if ($("msd-new-url").value.length > 0) {
			let html = "";
			let msg = "";
			try {
				let end = __http.responseText.lastIndexOf("\r\n");
				if (end < 0) {
					end = __http.responseText.length;
				}
				let begin = __http.responseText.lastIndexOf("\r\n", end - 2);
				if (begin < 0) {
					end = 0;
				}
				let result_str = __http.responseText.slice(begin, end);
				let result = JSON.parse(result_str);
				if (!result.ok) {
					html = "Can't upload image";
					msg = result_str;
				}
			} catch (ex) {
				html = "Can't parse upload result";
				msg = `${ex}`;
			}
			if (html.length > 0) {
				wm.error(html, msg);
			}
		}
		tools.hidden.setVisible($("msd-new-sub"), false);
		$("msd-new-file").value = "";
		$("msd-new-url").value = "";
		__http = null;
		__refreshControls();
	};

	var __clickAbortNewButton = function() {
		__http.onreadystatechange = null;
		__http.abort();
		__http = null;
		__refreshControls();
		tools.hidden.setVisible($("msd-new-sub"), true);
	};

	var __clickConnectButton = function(connected) {
		tools.httpPost("api/msd/set_connected", {"connected": connected}, function(http) {
			if (http.status !== 200) {
				wm.error("Can't switch Mass Storage", http.responseText);
			}
			__refreshControls();
		});
		__refreshControls();
		tools.el.setEnabled($(`msd-${connected ? "connect" : "disconnect"}-button`), false);
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset Mass Storage?").then(function(ok) {
			if (ok) {
				tools.httpPost("api/msd/reset", null, function(http) {
					if (http.status !== 200) {
						wm.error("Mass Storage reset error", http.responseText);
					}
				});
			}
		});
	};

	var __toggleSelectSub = function() {
		let el_sub = $("msd-new-sub");
		let visible = tools.hidden.isVisible(el_sub);
		tools.hidden.setVisible(el_sub, !visible);
		if (visible) {
			$("msd-new-file").value = "";
			$("msd-new-url").value = "";
		}
		__refreshControls();
	};

	var __selectNewFile = function() {
		let el = $("msd-new-file");
		let file = tools.input.getFile(el);
		if (file) {
			$("msd-new-url").value = "";
			if (__state && __state.storage && __state.storage.parts) {
				let part = __state.storage.parts[$("msd-new-part-selector").value];
				if (part && (file.size > part.size)) {
					let e_size = tools.escape(tools.formatSize(part.size));
					wm.error(`The new image is too big for the Mass Storage partition.<br>Maximum: ${e_size}`);
					el.value = "";
				}
			}
		}
		__refreshControls();
	};

	var __selectNewUrl = function() {
		if ($("msd-new-url").value.length > 0) {
			$("msd-new-file").value = "";
		}
		__refreshControls();
	};

	__init__();
}
