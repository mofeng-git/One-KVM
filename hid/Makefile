all:
	@ cat Makefile

build:
	platformio run

update:
	platformio platform update

install: upload
upload:
	platformio run --target upload

serial:
	platformio serialports monitor

regen:
	python3 genmap.py

clean:
	rm -rf .pioenvs .piolibdeps
