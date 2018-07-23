function main () {
	window.onclick = ui.windowClickHandler;
	session.loadKvmdVersion();
	session.startPoller();
	stream.startPoller();
}
