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


import {tools, $} from "../tools.js";
import {wm} from "../wm.js";


export function Msd() {
	var self = this;

	/************************************************************************/

	var __state = null;
	var __http = null;

	var __parts_names_json = "";
	var __parts_names_len = 0;
	var __parts = {};

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
				tools.httpPost(`/api/msd/remove?image=${name}`, function(http) {
					if (http.status !== 200) {
						wm.error("Can't remove image:<br>", http.responseText);
					}
				});
			}
		});
	};

	var __sendParam = function(name, value) {
		tools.httpPost(`/api/msd/set_params?${name}=${encodeURIComponent(value)}`, function(http) {
			if (http.status !== 200) {
				wm.error("Can't configure MSD:<br>", http.responseText);
			}
		});
	};

	var __clickUploadNewButton = function() {
		let file = tools.input.getFile($("msd-new-file"));
		__http = new XMLHttpRequest();
		let prefix = encodeURIComponent($("msd-new-part-selector").value);
		if (file) {
			__http.open("POST", `/api/msd/write?prefix=${prefix}&image=${encodeURIComponent(file.name)}&remove_incomplete=1`, true);
		} else {
			let url = $("msd-new-url").value;
			__http.open("POST", `/api/msd/write_remote?prefix=${prefix}&url=${encodeURIComponent(url)}&remove_incomplete=1`, true);
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
		tools.httpPost(`/api/msd/set_connected?connected=${connected}`, function(http) {
			if (http.status !== 200) {
				wm.error("Switch error:<br>", http.responseText);
			}
			__applyState();
		});
		__applyState();
		tools.el.setEnabled($(`msd-${connected ? "connect" : "disconnect"}-button`), false);
	};

	var __clickResetButton = function() {
		wm.confirm("Are you sure you want to reset Mass Storage Drive?").then(function(ok) {
			if (ok) {
				tools.httpPost("/api/msd/reset", function(http) {
					if (http.status !== 200) {
						wm.error("MSD reset error:<br>", http.responseText);
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
			let part = __state.storage.parts[$("msd-new-part-selector").value];
			if (file.size > part.size) {
				wm.error("New image is too big for the MSD partition.<br>Maximum:", tools.formatSize(part.size));
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
		__applyStateStatus();

		let s = __state;
		let online = (s && s.online);

		if (s) {
			tools.feature.setEnabled($("msd-dropdown"), s.enabled);
			tools.feature.setEnabled($("msd-reset-button"), s.enabled);
		}
		tools.hidden.setVisible($("msd-message-offline"), (s && !s.online));
		tools.hidden.setVisible($("msd-message-image-broken"), (online && s.drive.image && !s.drive.image.complete && !s.storage.uploading));
		tools.hidden.setVisible($("msd-message-too-big-for-cdrom"), (online && s.drive.cdrom && s.drive.image && s.drive.image.size >= 2359296000));
		tools.hidden.setVisible($("msd-message-out-of-storage"), (online && s.drive.image && !s.drive.image.in_storage));
		tools.hidden.setVisible($("msd-message-rw-enabled"), (online && s.drive.rw));
		tools.hidden.setVisible($("msd-message-another-user-uploads"), (online && s.storage.uploading && !__http));
		tools.hidden.setVisible($("msd-message-downloads"), (online && s.storage.downloading));

		if (online) {
			let names = Object.keys(s.storage.parts).sort();
			let parts_names_json = JSON.stringify(names);
			if (__parts_names_json !== parts_names_json) {
				$("msd-storages").innerHTML = names.map(name => `
					<div class="text">
						<div id="msd-storage-${tools.makeIdByText(name)}-progress" class="progress">
							<span class="progress-value"></span>
						</div>
					</div>
				`).join("<hr>");
				__parts_names_json = parts_names_json;
				__parts_names_len = names.length;
			}
			__parts = s.storage.parts;
		}
		for (let name in __parts) {
			let part = __parts[name];
			let title = (
				name.length === 0
				? `${__parts_names_len === 1 ? "Storage: %s" : "Internal storage: %s"}` // eslint-disable-line
				: `Storage [${name}${part.writable ? "]" : ", read-only]"}: %s` // eslint-disable-line
			);
			let id = `msd-storage-${tools.makeIdByText(name)}-progress`;
			if (online) {
				tools.progress.setSizeOf($(id), title, part.size, part.free);
			} else {
				tools.progress.setValue($(id), title.replace("%s", "unavailable"), 0);
			}
		}

		tools.el.setEnabled($("msd-image-selector"), (online && !s.drive.connected && !s.busy));
		__applyStateImageSelector();
		tools.el.setEnabled($("msd-download-button"), (online && s.drive.image && !s.drive.connected && !s.busy));
		tools.el.setEnabled($("msd-remove-button"), (online && s.drive.image && s.drive.image.removable && !s.drive.connected && !s.busy));

		tools.radio.setEnabled("msd-mode-radio", (online && !s.drive.connected && !s.busy));
		tools.radio.setValue("msd-mode-radio", `${Number(online && s.drive.cdrom)}`);

		tools.el.setEnabled($("msd-rw-switch"), (online && !s.drive.connected && !s.busy));
		$("msd-rw-switch").checked = (online && s.drive.rw);

		tools.el.setEnabled($("msd-connect-button"), (online && s.drive.image && !s.drive.connected && !s.busy));
		tools.el.setEnabled($("msd-disconnect-button"), (online && s.drive.connected && !s.busy));

		tools.el.setEnabled($("msd-select-new-button"), (online && !s.drive.connected && !__http && !s.busy));
		tools.el.setEnabled($("msd-upload-new-button"),
			(online && !s.drive.connected && (tools.input.getFile($("msd-new-file")) || $("msd-new-url").value.length > 0) && !s.busy));
		tools.el.setEnabled($("msd-abort-new-button"), (online && __http));

		tools.el.setEnabled($("msd-reset-button"), (s && s.enabled && !s.busy));

		tools.el.setEnabled($("msd-new-file"), (online && !s.drive.connected && !__http && !s.busy));
		tools.el.setEnabled($("msd-new-url"), (online && !s.drive.connected && !__http && !s.busy));
		tools.el.setEnabled($("msd-new-part-selector"), (online && !s.drive.connected && !__http && !s.busy));
		if (online && !s.storage.uploading && !s.storage.downloading) {
			let parts = Object.keys(s.storage.parts).sort().filter(name => (name === "" || s.storage.parts[name].writable));
			tools.selector.setValues($("msd-new-part-selector"), parts, "\u2500 Internal \u2500");
			tools.hidden.setVisible($("msd-new-part"), (parts.length > 1));
		}

		tools.hidden.setVisible($("msd-uploading-sub"), (online && s.storage.uploading));
		$("msd-uploading-name").innerHTML = ((online && s.storage.uploading) ? s.storage.uploading.name : "");
		$("msd-uploading-size").innerHTML = ((online && s.storage.uploading) ? tools.formatSize(s.storage.uploading.size) : "");
		if (online) {
			if (s.storage.uploading) {
				tools.progress.setPercentOf($("msd-uploading-progress"), s.storage.uploading.size, s.storage.uploading.written);
			} else if (!__http) {
				tools.progress.setValue($("msd-uploading-progress"), "Waiting for upload (press UPLOAD button) ...", 0);
			}
		} else {
			$("msd-new-file").value = "";
			$("msd-new-url").value = "";
			tools.progress.setValue($("msd-uploading-progress"), "", 0);
		}
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
		if (!(s && s.online) || s.storage.uploading || s.storage.downloading) {
			return;
		}

		let el = $("msd-image-selector");
		el.options.length = 1;

		let selected = "";

		for (let name of Object.keys(s.storage.images).sort()) {
			tools.selector.addSeparator(el);
			tools.selector.addOption(el, name, name);
			tools.selector.addComment(el, __makeImageSelectorInfo(s.storage.images[name]));
			if (s.drive.image && s.drive.image.name === name && s.drive.image.in_storage) {
				selected = name;
			}
		}

		if (s.drive.image && !s.drive.image.in_storage) {
			selected = ".__external";
			tools.selector.addOption(el, s.drive.image.name, selected);
			tools.selector.addComment(el, __makeImageSelectorInfo(s.drive.image));
		}

		el.value = selected;
	};

	var __makeImageSelectorInfo = function(image) {
		let info = `\xA0\xA0\xA0\xA0\xA0\u2570 ${tools.formatSize(image.size)}`;
		info += (image.complete ? "" : ", broken");
		if (image.in_storage !== undefined && !image.in_storage) {
			info += ", out of storage";
		}
		let dt = new Date(image.mod_ts * 1000);
		dt = new Date(dt.getTime() - (dt.getTimezoneOffset() * 60000));
		info += " \u2500 " + dt.toISOString().slice(0, -8).replaceAll("-", ".").replace("T", "-");
		return info;
	};

	__init__();
}
