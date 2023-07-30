serial:
	make _build E=serial C=avr
spi:
	make _build E=spi C=avr
aum:
	make _build E=aum C=avr
stm32:
	platformio run --environment patch --project-conf platformio-stm32.ini
	make _build E=serial C=stm32
_build:
	rm -f .current .config
	platformio run --environment $(E) --project-conf platformio-$(C).ini
	echo -n $(E) > .current
	echo -n $(C) > .config

# Added to easy test all builds
_build_all: aum spi serial stm32
	rm -f .current .config

install: upload
upload:
	$(eval $@_CURRENT := $(shell cat .current))
	$(eval $@_CONFIG := $(shell cat .config))
	bash -ex -c " \
		current=`cat .current`; \
		if [ '$($@_CURRENT)' == 'spi' ] || [ '$($@_CURRENT)' == 'aum' ]; then \
			gpioset 0 25=1; \
			gpioset 0 25=0; \
		fi \
	"
	platformio run --environment '$($@_CURRENT)' --project-conf 'platformio-$($@_CONFIG).ini' --target upload


bootloader-spi: install-bootloader-spi
install-bootloader-spi: upload-bootloader-spi
upload-bootloader-spi:
	platformio run --environment bootloader_spi --project-conf platformio-avr.ini --target bootloader


update:
	platformio platform update


clean-all: clean
	rm -rf .platformio
clean:
	rm -rf .pio .current .config platformio.ini


help:
	@ cat Makefile
