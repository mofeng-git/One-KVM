function check_browser() {
	if (
		!window.navigator
		|| window.navigator.userAgent.indexOf("MSIE ") > 0
		|| window.navigator.userAgent.indexOf("Trident/") > 0
		|| window.navigator.userAgent.indexOf("Edge/") > 0
	) {
		$("bad-browser-modal").style.visibility = "visible";
		return false;
	} else {
		return true;
	}
}
