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


var wm;

function main() {
	if (checkBrowser()) {
		wm = new WindowManager();

		tools.setOnClick($("show-about-button"), () => wm.showWindow($("about-window")));
		tools.setOnClick($("show-keyboard-button"), () => wm.showWindow($("keyboard-window")));
		tools.setOnClick($("show-stream-button"), () => wm.showWindow($("stream-window")));
		tools.setOnClick($("open-log-button"), () => window.open("/kvmd/log?seek=3600&follow=1", "_blank"));

		wm.showWindow($("stream-window"));

		new Session();
	}
}
