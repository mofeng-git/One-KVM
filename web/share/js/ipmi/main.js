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


import {$, tools} from "../tools.js";


export function main() {
	__loadKvmdInfo();
}

function __loadKvmdInfo() {
	tools.httpGet("api/info", null, function(http) {
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
	let make_item = function (comment, cmd, api) {
		return `
			<span class="code-comment">
				# ${tools.escape(comment)}:<br>$
			</span>
			ipmitool -I lanplus -U admin -P admin
				-H ${tools.escape(window.location.hostname)}
				-p ${tools.escape(info.extras.ipmi.port)} ${tools.escape(cmd)}
			<br>
			<span class="code-comment">$</span>
			curl -XPOST -HX-KVMD-User:admin -HX-KVMD-Passwd:admin -k \\<br>&nbsp;&nbsp;&nbsp;&nbsp;
			${tools.escape(window.location.protocol + "//" + window.location.host + "/api/atx" + api)}
		`;
	};
	$("ipmi-text").innerHTML = [
		make_item("Power on the server if it's off",		"power on",		"/power?action=on"),
		make_item("Soft power off the server if it's on",	"power soft",	"/power?action=off"),
		make_item("Hard power off the server if it's on",	"power off",	"/power?action=off_hard"),
		make_item("Hard reset the server if it's on",		"power reset",	"/power?action=reset_hard"),
		make_item("Check the power status",					"power status",	""),
	].join("<br><br>");
}
