function main() {
	var hid = new Hid();
	new Session(new Atx(), hid, new Msd());
	new Stream();
	new Ui(hid);
}
