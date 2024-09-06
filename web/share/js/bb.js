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


export var browser = new function() {
	// https://stackoverflow.com/questions/9847580/how-to-detect-safari-chrome-ie-firefox-and-opera-browser/9851769
	// https://github.com/fingerprintjs/fingerprintjs/discussions/641

	// Opera 8.0+
	let is_opera = (
		(!!window.opr && !!opr.addons) // eslint-disable-line no-undef
		|| !!window.opera
		|| (navigator.userAgent.indexOf(" OPR/") >= 0)
	);

	// Firefox 1.0+
	let is_firefox = (typeof mozInnerScreenX !== "undefined");

	// Safari 3.0+ "[object HTMLElementConstructor]"
	let is_safari = (function() {
		if (/constructor/i.test(String(window["HTMLElement"]))) {
			return true;
		}
		let push = null;
		try {
			push = window.top["safari"].pushNotification;
		} catch {
			try {
				push = window["safari"].pushNotification;
			} catch {
				return false;
			}
		}
		return String(push) === "[object SafariRemoteNotification]";
	})();

	// Chrome 1+
	let is_chrome = !!window.chrome;

	// Blink engine detection
	let is_blink = ((is_chrome || is_opera) && !!window.CSS);

	// Any browser on Mac
	let is_mac = ((
		window.navigator.oscpu
		|| window.navigator.platform
		|| window.navigator.appVersion
		|| "Unknown"
	).indexOf("Mac") !== -1);

	// Any Windows
	let is_win = (navigator && !!(/win/i).exec(navigator.platform));

	// iOS browsers
	// https://stackoverflow.com/questions/9038625/detect-if-device-is-ios
	// https://github.com/lancedikson/bowser/issues/329
	let is_ios = (!!navigator.platform && (
		/iPad|iPhone|iPod/.test(navigator.platform)
		|| (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1 && !window["MSStream"])
	));

	let is_android = /android/i.test(navigator.userAgent);

	let flags = {
		"is_opera": is_opera,
		"is_firefox": is_firefox,
		"is_safari": is_safari,
		"is_chrome": is_chrome,
		"is_blink": is_blink,
		"is_mac": is_mac,
		"is_win": is_win,
		"is_ios": is_ios,
		"is_android": is_android,
		"is_mobile": (is_ios || is_android),
	};

	console.log("===== BB flags:", flags);
	return flags;
};

export function checkBrowser(desktop_css, mobile_css) {
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
		let force_desktop = (new URL(window.location.href)).searchParams.get("force_desktop");
		let force_mobile = (new URL(window.location.href)).searchParams.get("force_mobile");
		if ((force_desktop || !browser.is_mobile) && !force_mobile) {
			__addCssLink("/share/css/x-desktop.css");
			if (desktop_css) {
				__addCssLink(desktop_css);
			}
		} else {
			__addCssLink("/share/css/x-mobile.css");
			if (mobile_css) {
				__addCssLink(mobile_css);
			}
		}
		return true;
	}
}

function __addCssLink(path) {
	console.log("===== Adding CSS:", path);
	let el_head = document.getElementsByTagName("head")[0];
	let el_link = document.createElement("link");
	el_link.rel = "stylesheet";
	el_link.type = "text/css";
	el_link.href = path;
	el_link.media = "all";
	el_head.appendChild(el_link);
}
