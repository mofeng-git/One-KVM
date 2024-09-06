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

import {Session} from "./session.js";


export function main() {
	if (checkBrowser(null, "/share/css/kvm/x-mobile.css")) {
		tools.storage.bindSimpleSwitch($("page-close-ask-switch"), "page.close.ask", true, function(value) {
			if (value) {
				window.onbeforeunload = function(event) {
					let text = "Are you sure you want to close PiKVM session?";
					if (event) {
						event.returnValue = text;
					}
					return text;
				};
			} else {
				window.onbeforeunload = null;
			}
		});

		initWindowManager();

		tools.el.setOnClick($("open-log-button"), () => window.open("/api/log?seek=3600&follow=1", "_blank"));

		if (tools.config.getBool("kvm--full-tab-stream", false)) {
			wm.toggleFullTabWindow($("stream-window"), true);
		}
		wm.showWindow($("stream-window"));

		new Session();
	}
}
