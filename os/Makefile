all:
	cat Makefile

v1:
	make _pikvm PIKVM=v1

_pikvm: pi-builder
	rm -rf pi-builder/builder/scripts/pikvm
	cp -a platforms/$(PIKVM_PLATFORM) pi-builder/builder/scripts/pikvm
	cd pi-builder && make _rpi \
		PLATFORM=rpi-2 \
		BUILDER=pikvm-builder \
		STAGES="base keys watchdog ro pikvm"

pi-builder:
	git clone https://github.com/mdevaev/pi-builder --depth=1

format: pi-builder
	cd pi-builder && make format

install: pi-builder
	cd pi-builder && make install

scan: pi-builder
	cd pi-builder && make scan

clean: pi-builder
	cd pi-builder && make clean

clean-all:
	-cd pi-builder && make clean
	rm -rf pi-builder
