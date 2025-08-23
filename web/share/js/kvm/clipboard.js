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


import {tools} from "../tools.js";
import {wm} from "../wm.js";


export var clipboard = new function() {
	var self = this;

	/************************************************************************/

	self.setText = function(text) {
		let workaround = function(ex) {
			// https://stackoverflow.com/questions/60317969/document-execcommandcopy-not-working-even-though-the-dom-element-is-created
			wm.info("Press OK to copy the text to the clipboard").then(function() {
				tools.error("clipboard.setText(): navigator.clipboard.writeText() is not working:", ex);
				tools.info("clipboard.setText(): Trying a workaround...");

				let el = document.createElement("textarea");
				el.readonly = true;
				el.contentEditable = true;
				el.style.position = "absolute";
				el.style.top = "-1000px";
				el.value = text;
				document.body.appendChild(el);

				// Select the content of the textarea
				el.select(); // Ordinary browsers
				el.setSelectionRange(0, el.value.length); // iOS

				try {
					ex = (document.execCommand("copy") ? null : "Unknown error");
				} catch (ex) { // eslint-disable-line no-unused-vars
				}

				// Remove the added textarea again:
				document.body.removeChild(el);

				if (ex) {
					tools.error("clipboard.setText(): Workaround failed:", ex);
					wm.error("Can't copy text to the clipboard", `${ex}`);
				}
			});
		};
		if (navigator.clipboard) {
			navigator.clipboard.writeText(text).then(function() {
				wm.info("The text has been copied to the clipboard");
			}, function(ex) {
				workaround(ex);
			});
		} else {
			workaround("navigator.clipboard is not available");
		}
	};
};
