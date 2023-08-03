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


.pico-sdk:
	rm -rf .pico-sdk.tmp
	git clone https://github.com/raspberrypi/pico-sdk .pico-sdk.tmp
	cd .pico-sdk.tmp \
		&& git checkout 62201a83e2693ea165fdc7669b4ab2f3b4f43c36 \
		&& git submodule update --init
	mv .pico-sdk.tmp .pico-sdk


.tinyusb:
	rm -rf .tinyusb.tmp
	git clone https://github.com/hathach/tinyusb .tinyusb.tmp
	cd .tinyusb.tmp \
		&& git checkout c998e9c60bc76894006c3bd049d661124a9bfbfd \
		&& git submodule update --init
	mv .tinyusb.tmp .tinyusb
