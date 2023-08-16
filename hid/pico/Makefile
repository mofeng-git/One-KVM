all: deps
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
	rm -rf .pico-sdk* .tinyusb*


define libdep
	rm -rf .$(1).tmp
	git clone https://github.com/$(2) .$(1).tmp
	cd .$(1).tmp \
		&& git checkout $(3) \
		&& (test ! -f .gitmodules || git submodule update --init)
	mv .$(1).tmp .$(1)
endef
.pico-sdk:
	$(call libdep,pico-sdk,raspberrypi/pico-sdk,6a7db34ff63345a7badec79ebea3aaef1712f374)
.tinyusb:
	$(call libdep,tinyusb,hathach/tinyusb,d713571cd44f05d2fc72efc09c670787b74106e0)
# TODO: PS2: Add your library here and add it to "all" and "clean-all" targets
deps: .pico-sdk .tinyusb


.PHONY: deps
