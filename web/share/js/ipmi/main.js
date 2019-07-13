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


import {$} from "../tools.js";


export function main() {
	let host = window.location.hostname;
	let site = window.location.protocol + "//" + window.location.host;
	$("ipmi-text").innerHTML = `
		<span class="code-comment"># Power on the server if it's off:<br>
		$</span> ipmitool -I lanplus -U admin -P admin -H ${host} power on<br>
		<span class="code-comment">$</span> curl -XPOST -HX-KVMD-User:admin -HX-KVMD-Passwd:admin -k \\<br>
		&nbsp;&nbsp;&nbsp;&nbsp;${site}/api/atx/power?action=on<br>
		<br>
		<span class="code-comment"># Soft power off the server if it's on:<br>
		$</span> ipmitool -I lanplus -U admin -P admin -H ${host} power soft<br>
		<span class="code-comment">$</span> curl -XPOST -HX-KVMD-User:admin -HX-KVMD-Passwd:admin -k \\<br>
		&nbsp;&nbsp;&nbsp;&nbsp;${site}/api/atx/power?action=off<br>
		<br>
		<span class="code-comment"># Hard power off the server if it's on:<br>
		$</span> ipmitool -I lanplus -U admin -P admin -H ${host} power off<br>
		<span class="code-comment">$</span> curl -XPOST -HX-KVMD-User:admin -HX-KVMD-Passwd:admin -k \\<br>
		&nbsp;&nbsp;&nbsp;&nbsp;${site}/api/atx/power?action=off_hard<br>
		<br>
		<span class="code-comment"># Hard reset the server if it's on:<br>
		$</span> ipmitool -I lanplus -U admin -P admin -H ${host} power reset<br>
		<span class="code-comment">$</span> curl -XPOST -HX-KVMD-User:admin -HX-KVMD-Passwd:admin -k \\<br>
		&nbsp;&nbsp;&nbsp;&nbsp;${site}/api/atx/power?action=reset_hard<br>
		<br>
		<span class="code-comment"># Check the power status:<br>
		$</span> ipmitool -I lanplus -U admin -P admin -H ${host} power status<br>
		<span class="code-comment">$</span> curl -HX-KVMD-User:admin -HX-KVMD-Passwd:admin -k \\<br>
		&nbsp;&nbsp;&nbsp;&nbsp;${site}/api/atx
	`;
}
