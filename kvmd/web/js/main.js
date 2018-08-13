function main() {
	var hid = new Hid();
	var ui = new Ui(hid);
	new Session(new Atx(), hid, new Msd());
	new Stream(ui);
}
