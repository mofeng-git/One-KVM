usb:
	make _build E=usb
ps2:
	make _build E=ps2
_build:
	rm -f .current
	platformio run --environment $(E)
	echo -n $(E) > .current


install: upload
upload:
	platformio run --environment $(shell cat .current) --target upload


update:
	platformio platform update


clean-all: clean
clean:
	rm -rf .pio .current


help:
	@ cat Makefile
