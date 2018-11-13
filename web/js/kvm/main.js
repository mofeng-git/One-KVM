var ui;

function main() {
	if (
		!window.navigator
		|| window.navigator.userAgent.indexOf("MSIE ") > 0
		|| window.navigator.userAgent.indexOf("Trident/") > 0
		|| window.navigator.userAgent.indexOf("Edge/") > 0
	) {
		$("bad-browser-modal").style.visibility = "visible";
	} else {
		ui = new Ui();

		tools.setOnClick($("show-about-button"), () => ui.showWindow($("about-window")));
		tools.setOnClick($("show-keyboard-button"), () => ui.showWindow($("keyboard-window")));
		tools.setOnClick($("show-stream-button"), () => ui.showWindow($("stream-window")));
		tools.setOnClick($("open-log-button"), () => ui.open("kvmd/log?seek=3600&follow=1", "_blank"));

		ui.showWindow($("stream-window"));

		new Session();
	}
}
