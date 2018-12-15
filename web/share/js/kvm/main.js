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
