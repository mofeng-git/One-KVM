function main() {
	if (
		!window.navigator
		|| window.navigator.userAgent.indexOf("MSIE ") > 0
		|| window.navigator.userAgent.indexOf("Trident/") > 0
		|| window.navigator.userAgent.indexOf("Edge/") > 0
	) {
		$("bad-browser-modal").style.visibility = "visible";
	} else {
		var hid = new Hid();
		var ui = new Ui(hid);
		new Session(new Atx(), hid, new Msd());
		new Stream(ui);
	}
}
