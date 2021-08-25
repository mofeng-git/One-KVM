serial:
	make _build E=serial
spi:
	make _build E=spi
aum:
	make _build E=aum
_build:
	rm -f .current
	platformio run --environment $(E)
	echo -n $(E) > .current


install: upload
upload:
	$(eval $@_CURRENT := $(shell cat .current))
	bash -ex -c " \
		current=`cat .current`; \
		if [ '$($@_CURRENT)' == 'spi' ] || [ '$($@_CURRENT)' == 'aum' ]; then \
			gpioset 0 25=1; \
			gpioset 0 25=0; \
		fi \
	"
	platformio run --environment '$($@_CURRENT)' --target upload


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
