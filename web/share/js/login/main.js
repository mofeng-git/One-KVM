/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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
	if (checkBrowser(null, null)) {
		initWindowManager();

		tools.el.setOnClick($("login-button"), __login);
		$("user-input").onkeyup = $("passwd-input").onkeyup = function(event) {
			if (event.code === "Enter") {
				event.preventDefault();
				$("login-button").click();
			}
		};

		$("user-input").focus();
	}
}

function __login() {
	let user = $("user-input").value;
	if (user.length === 0) {
		$("user-input").focus();
	} else {
		let passwd = $("passwd-input").value;
		let body = `user=${encodeURIComponent(user)}&passwd=${encodeURIComponent(passwd)}`;
		let http = tools.makeRequest("POST", "/api/auth/login", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					document.location.href = "/";
				} else if (http.status === 403) {
					wm.error("Invalid username or password").then(__tryAgain);
				} else {
					let error = "";
					if (http.status === 400) {
						try { error = JSON.parse(http.responseText)["result"]["error"]; } catch (_) { /* Nah */ }
					}
					if (error === "ValidatorError") {
						wm.error("Invalid username or password characters").then(__tryAgain);
					} else {
						wm.error("Login error:<br>", http.responseText).then(__tryAgain);
					}
				}
			}
		}, body, "application/x-www-form-urlencoded");
		__setEnabled(false);
	}
}

function __setEnabled(enabled) {
	tools.el.setEnabled($("user-input"), enabled);
	tools.el.setEnabled($("passwd-input"), enabled);
	tools.el.setEnabled($("login-button"), enabled);
}

function __tryAgain() {
	__setEnabled(true);
	$("passwd-input").focus();
	$("passwd-input").select();
}
