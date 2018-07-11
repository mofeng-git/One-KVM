all:
	@ cat Makefile

build:
	platformio run

update:
	platformio platform update

upload:
	platformio run --target upload

serial:
	platformio serialports monitor

clean:
	rm -rf .pioenvs .piolibdeps
