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
		new Session(new Hid(), new Atx(), new Msd(), new Streamer());
	}
}
