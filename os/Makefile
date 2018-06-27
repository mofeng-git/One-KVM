all:
	cat Makefile

pi-builder:
	git clone https://github.com/mdevaev/pi-builder --depth=1

pi-kvm: pi-builder
	rm -rf pi-builder/builder/scripts/kvm
	cp -a kvm pi-builder/builder/scripts
	cd pi-builder && make _rpi \
		PLATFORM=rpi-2 \
		BUILDER=kvm-builder \
		STAGES="base keys watchdog ro kvm"

format: pi-builder
	cd pi-builder && make format

install: pi-builder
	cd pi-builder && make install

clean: pi-builder
	cd pi-builder && make clean

clean-all: clean
	rm -rf pi-builder
