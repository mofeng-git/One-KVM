var ui;

function main() {
	if (check_browser()) {
		ui = new Ui();

		tools.setOnClick($("show-about-button"), () => ui.showWindow($("about-window")));
		tools.setOnClick($("show-keyboard-button"), () => ui.showWindow($("keyboard-window")));
		tools.setOnClick($("show-stream-button"), () => ui.showWindow($("stream-window")));
		tools.setOnClick($("open-log-button"), () => window.open("kvmd/log?seek=3600&follow=1", "_blank"));

		ui.showWindow($("stream-window"));

		new Session();
	}
}
