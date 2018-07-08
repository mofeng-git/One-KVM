_BUILD_DIR=./.build

all:
	@ cat Makefile

v1:
	make _pikvm PIKVM_PLATFORM=v1

_pikvm: $(_BUILD_DIR)
	rm -rf $(_BUILD_DIR)/stages/pikvm
	rm -rf $(_BUILD_DIR)/builder/scripts/pikvm
	cp -a platforms/$(PIKVM_PLATFORM) $(_BUILD_DIR)/stages/pikvm
	cd $(_BUILD_DIR) && make binfmt && make _rpi \
		BUILD_OPTS="--build-arg KVMD_VERSION=`bash -c '. ../kvmd/PKGBUILD; echo $$pkgver'`" \
		PROJECT=pi-kvm \
		PLATFORM=rpi-2 \
		STAGES="__init__ os ssh watchdog ro pikvm __cleanup__" \
		HOSTNAME=pikvm

$(_BUILD_DIR):
	git clone --depth=1 https://github.com/mdevaev/pi-builder $(_BUILD_DIR)

install: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make install

scan: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make scan

clean: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make clean

clean-all:
	-cd $(_BUILD_DIR) && make clean-all
	rm -rf $(_BUILD_DIR)
