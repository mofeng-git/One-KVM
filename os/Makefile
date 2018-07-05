_BUILD_DIR=./.build

all:
	cat Makefile

v1:
	make _pikvm PIKVM_PLATFORM=v1

_pikvm: $(_BUILD_DIR)
	rm -rf $(_BUILD_DIR)/stages/pikvm
	rm -rf $(_BUILD_DIR)/builder/scripts/pikvm
	cp -a platforms/$(PIKVM_PLATFORM) $(_BUILD_DIR)/stages/pikvm
	cd $(_BUILD_DIR) && make _rpi \
		PLATFORM=rpi-2 \
		BUILDER=pikvm-builder \
		STAGES="base os ssh watchdog ro pikvm" \
		HOSTNAME=pikvm

$(_BUILD_DIR):
	git clone --depth=1 https://github.com/mdevaev/pi-builder $(_BUILD_DIR)

format: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make format

install: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make install

scan: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make scan

clean: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make clean

clean-all:
	-cd $(_BUILD_DIR) && make clean-all
	rm -rf $(_BUILD_DIR)
