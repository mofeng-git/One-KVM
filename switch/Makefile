all:
	@echo "Run 'make install'"

upload: install
install:
	mount `python -m kvmd.apps.swctl bootloader 0` mnt
	cp switch.uf2 mnt
	umount mnt
