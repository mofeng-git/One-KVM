all: .pico-sdk .tinyusb
	rm -f hid.uf2
	cmake -B .build
	cmake --build .build --config Release -- -j
	ln .build/src/hid.uf2 .


upload: install
install: all
	sudo mount /dev/sda1 /mnt
	sudo cp hid.uf2 /mnt
	sudo umount /mnt


clean:
	rm -rf .build hid.uf2
clean-all: clean
	rm -rf .pico-sdk.tmp .pico-sdk .tinyusb.tmp .tinyusb


define libdep
	rm -rf .$(1).tmp
	git clone https://github.com/$(2) .$(1).tmp
	cd .$(1).tmp \
		&& git checkout $(3) \
		&& (test ! -f .gitmodules || git submodule update --init)
	mv .$(1).tmp .$(1)
endef
.pico-sdk:
	$(call libdep,pico-sdk,raspberrypi/pico-sdk,62201a83e2693ea165fdc7669b4ab2f3b4f43c36)
.tinyusb:
	$(call libdep,tinyusb,hathach/tinyusb,c998e9c60bc76894006c3bd049d661124a9bfbfd)
# TODO: PS2: Add your library here and add it to "all" and "clean-all" targets
