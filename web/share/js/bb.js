/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
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


export function checkBrowser() {
	if (
		!window.navigator
		|| window.navigator.userAgent.indexOf("MSIE ") > 0
		|| window.navigator.userAgent.indexOf("Trident/") > 0
		|| window.navigator.userAgent.indexOf("Edge/") > 0
	) {
		let el_modal = document.createElement("div");
		el_modal.className = "modal";
		el_modal.style.visibility = "visible";
		el_modal.innerHTML = `
			<div class="modal-window">
				<div class="modal-content">
					Hello. You are using an incompatible or legacy browser.<br>
					Please use one of the following browsers:
					<hr>
					<ul>
						<li><a target="_blank" href="https://google.com/chrome">Google Chrome</a> <sup><i>recommended</i></sup></li>
						<li><a target="_blank" href="https://chromium.org/Home">Chromium</a> <sup><i>recommended</i></sup></li>
						<li><a target="_blank" href="https://mozilla.org/firefox">Mozilla Firefox</a></li>
						<li><a target="_blank" href="https://apple.com/safari">Apple Safari</a></li>
						<li><a target="_blank" href="https://opera.com">Opera</a></li>
						<li><a target="_blank" href="https://vivaldi.com">Vivaldi</a></li>
					</ul>
				</div>
			</div>
		`;
		document.body.appendChild(el_modal);
		return false;
	} else {
		return true;
	}
}
