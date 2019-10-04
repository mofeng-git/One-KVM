all:
	platformio run --environment $(if $(E),$(E),serial_prod)

update:
	platformio platform update

install: upload
upload:
	platformio run --environment $(if $(E),$(E),serial_prod) --target upload

clean-all: clean
clean:
	rm -rf .pio

help:
	@ cat Makefile
