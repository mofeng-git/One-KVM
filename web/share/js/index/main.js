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
	let e_href = tools.escape(window.location.href);
	$("app-text").innerHTML = `
		<span class="code-comment"># On Linux using Chromium/Chrome via any terminal:<br>
		$</span> \`which chromium 2>/dev/null || which chrome 2>/dev/null || which google-chrome\` --app="${e_href}"<br>
		<br>
		<span class="code-comment"># On MacOS using Terminal application:<br>
		$</span> /Applications/Google&bsol; Chrome.app/Contents/MacOS/Google&bsol; Chrome --app="${e_href}"<br>
		<br>
		<span class="code-comment"># On Windows via cmd.exe:<br>
		C:&bsol;&gt;</span> start chrome --app="${e_href}"
	`;
}

function __loadKvmdInfo() {
	tools.httpGet("api/info", {"fields": "auth,meta,extras"}, function(http) {
		switch (http.status) {
			case 200:
				__showKvmdInfo(JSON.parse(http.responseText).result);
				break;

			case 401:
			case 403:
				tools.currentOpen("login");
				break;

			default:
				setTimeout(__loadKvmdInfo, 1000);
				break;
		}
	});
}

function __showKvmdInfo(info) {
	let apps = [];
	if (info.extras === null) {
		wm.error("Not all applications in the menu can be displayed due an error.<br>See KVMD logs for details.");
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

	let html = "";

	// Don't use this option, it may be removed in any time
	let hide_kvm_button = (
		(info.meta !== null && info.meta.web && info.meta.web.hide_kvm_button)
		|| tools.config.getBool("index--hide-kvm-button", false)
	);
	if (!hide_kvm_button) {
		html += __makeApp(null, "kvm", "share/svg/kvm.svg", "KVM");
	}

	for (let app of apps) {
		if (app.place >= 0 && (app.enabled || app.started)) {
			html += __makeApp(null, app.path, app.icon, app.name);
		}
	}

	if (info.auth.enabled) {
		html += __makeApp("logout-button", "#", "share/svg/logout.svg", "Logout");
	}

	$("apps-box").innerHTML = `<ul id="apps">${html}</ul>`;

	if (info.auth.enabled) {
		tools.el.setOnClick($("logout-button"), __logout);
	}

	if (info.meta !== null && info.meta.server && info.meta.server.host) {
		$("kvmd-meta-server-host").innerHTML = info.meta.server.host;
		document.title = `PiKVM Index: ${info.meta.server.host}`;
	} else {
		$("kvmd-meta-server-host").innerHTML = "";
		document.title = "PiKVM Index";
	}
}

function __makeApp(id, path, icon, name) {
	// Tailing slash in href is added to avoid Nginx 301 redirect
	// when the location doesn't have tailing slash: "foo -> foo/".
	// Reverse proxy over PiKVM can be misconfigured to handle this.
	let e_add_id = (id ? `id="${tools.escape(id)}"` : "");
	return `<li>
		<div ${e_add_id} class="app">
			<a href="${tools.escape(ROOT_PREFIX + path)}/">
				<div>
					<img class="svg-gray" src="${tools.escape(ROOT_PREFIX + icon)}">
					${tools.escape(name)}
				</div>
			</a>
		</div>
	</li>`;
}

function __logout() {
	tools.httpPost("api/auth/logout", null, function(http) {
		switch (http.status) {
			case 200:
			case 401:
			case 403:
				tools.currentOpen("login");
				break;

			default:
				wm.error("Logout error", http.responseText);
				break;
		}
	});
}
