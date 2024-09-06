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
	tools.httpGet("/api/info", function(http) {
		if (http.status === 200) {
			let ipmi_port = JSON.parse(http.responseText).result.extras.ipmi.port;
			let make_item = (comment, ipmi, api) => `
				<span class="code-comment"># ${comment}:<br>$</span>
				ipmitool -I lanplus -U admin -P admin -H ${window.location.hostname} -p ${ipmi_port} ${ipmi}<br>
				<span class="code-comment">$</span> curl -XPOST -HX-KVMD-User:admin -HX-KVMD-Passwd:admin -k \\<br>
				&nbsp;&nbsp;&nbsp;&nbsp;${window.location.protocol}//${window.location.host}/api/atx${api}<br>
			`;
			$("ipmi-text").innerHTML = `
				${make_item("Power on the server if it's off", "power on", "/power?action=on")}
				<br>
				${make_item("Soft power off the server if it's on", "power soft", "/power?action=off")}
				<br>
				${make_item("Hard power off the server if it's on", "power off", "/power?action=off_hard")}
				<br>
				${make_item("Hard reset the server if it's on", "power reset", "/power?action=reset_hard")}
				<br>
				${make_item("Check the power status", "power status", "")}
			`;
		} else if (http.status === 401 || http.status === 403) {
			document.location.href = "/login";
		} else {
			setTimeout(__loadKvmdInfo, 1000);
		}
	});
}
