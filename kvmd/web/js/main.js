function main () {
	ui.init();
	hid.init();
	session.loadKvmdVersion();
	session.startPoller();
	stream.startPoller();
}
