function main () {
	window.onclick = ui.windowClickHandler;
	session.startPoller();
	stream.startPoller();
}
