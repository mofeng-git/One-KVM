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
import {checkBrowser} from "../bb.js";
import {wm, initWindowManager} from "../wm.js";


export function main() {
	initWindowManager();

	if (checkBrowser(null, null)) {
		__setAppText();
		__loadKvmdInfo();
	}
}

function __setAppText() {
	$("app-text").innerHTML = `
		<span class="code-comment"># On Linux using Chromium/Chrome via any terminal:<br>
		$</span> \`which chromium 2>/dev/null || which chrome 2>/dev/null || which google-chrome\` --app="${window.location.href}"<br>
		<br>
		<span class="code-comment"># On MacOS using Terminal application:<br>
		$</span> /Applications/Google&bsol; Chrome.app/Contents/MacOS/Google&bsol; Chrome --app="${window.location.href}"<br>
		<br>
		<span class="code-comment"># On Windows via cmd.exe:<br>
		C:&bsol;&gt;</span> start chrome --app="${window.location.href}"
	`;
}

function __loadKvmdInfo() {
	tools.httpGet("/api/info?fields=auth,meta,extras", function(http) {
		if (http.status === 200) {
			let info = JSON.parse(http.responseText).result;

			let apps = [];
			if (info.extras === null) {
				wm.error("Not all applications in the menu can be displayed<br>due an error. See KVMD logs for details.");
			} else {
				apps = Object.values(info.extras).sort(function(a, b) {
					if (a.place < b.place) {
						return -1;
					} else if (a.place > b.place) {
						return 1;
					} else {
						return 0;
					}
				});
			}

			$("apps-box").innerHTML = "<ul id=\"apps\"></ul>";

			// Don't use this option, it may be removed in any time
			let hide_kvm_button = (
				(info.meta !== null && info.meta.web && info.meta.web.hide_kvm_button)
				|| tools.config.getBool("index--hide-kvm-button", false)
			);
			if (!hide_kvm_button) {
				$("apps").innerHTML += __makeApp(null, "kvm", "share/svg/kvm.svg", "KVM");
			}

			for (let app of apps) {
				if (app.place >= 0 && (app.enabled || app.started)) {
					$("apps").innerHTML += __makeApp(null, app.path, app.icon, app.name);
				}
			}

			if (info.auth.enabled) {
				$("apps").innerHTML += __makeApp("logout-button", "#", "share/svg/logout.svg", "Logout");
				tools.el.setOnClick($("logout-button"), __logout);
			}

			if (info.meta !== null && info.meta.server && info.meta.server.host) {
				$("kvmd-meta-server-host").innerHTML = info.meta.server.host;
				document.title = `PiKVM Index: ${info.meta.server.host}`;
			} else {
				$("kvmd-meta-server-host").innerHTML = "";
				document.title = "PiKVM Index";
			}
		} else if (http.status === 401 || http.status === 403) {
			document.location.href = "/login";
		} else {
			setTimeout(__loadKvmdInfo, 1000);
		}
	});
}

function __makeApp(id, path, icon, name) {
	return `<li>
		<div ${id ? "id=\"" + id + "\"" : ""} class="app">
			<a href="${path}">
				<div>
					<img class="svg-gray" src="${icon}">
					${name}
				</div>
			</a>
		</div>
	</li>`;
}

function __logout() {
	tools.httpPost("/api/auth/logout", function(http) {
		if (http.status === 200 || http.status === 401 || http.status === 403) {
			document.location.href = "/login";
		} else {
			wm.error("Logout error:<br>", http.responseText);
		}
	});
}
