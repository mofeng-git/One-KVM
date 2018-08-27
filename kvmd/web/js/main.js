function main() {
	if (
		!window.navigator
		|| window.navigator.userAgent.indexOf("MSIE ") > 0
		|| window.navigator.userAgent.indexOf("Trident/") > 0
		|| window.navigator.userAgent.indexOf("Edge/") > 0
	) {
		$("bad-browser-modal").style.visibility = "visible";
	} else {
		var ui = new Ui();
		new Session(new Atx(), new Hid(), new Msd());
		new Stream(ui);
	}
}
