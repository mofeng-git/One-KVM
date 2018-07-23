function main () {
	ui.init();
	session.loadKvmdVersion();
	session.startPoller();
	stream.startPoller();
}
