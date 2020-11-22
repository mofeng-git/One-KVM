serial:
	make _build E=serial
spi:
	make _build E=spi
_build:
	rm -f .current
	platformio run --environment $(E)
	echo -n $(E) > .current


install: upload
upload:
	platformio run --environment $(shell cat .current) --target upload


bootloader-spi: install-bootloader-spi
install-bootloader-spi: upload-bootloader-spi
upload-bootloader-spi:
	platformio run --environment bootloader_spi --target bootloader


update:
	platformio platform update


clean-all: clean
	rm -rf .platformio
clean:
	rm -rf .pio .current


help:
	@ cat Makefile
