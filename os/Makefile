_BUILD_DIR = ./.build
_KVMD_VERSION = $(shell bash -c 'source ../kvmd/PKGBUILD; echo $$pkgver')

all:
	@ cat Makefile

v1:
	make _pikvm PIKVM_PLATFORM=v1

shell:
	cd $(_BUILD_DIR) && make shell

_pikvm: $(_BUILD_DIR)
	rm -rf $(_BUILD_DIR)/stages/pikvm
	rm -rf $(_BUILD_DIR)/builder/scripts/pikvm
	cp -a platforms/$(PIKVM_PLATFORM) $(_BUILD_DIR)/stages/pikvm
	cd $(_BUILD_DIR) && make binfmt && make _rpi \
		BUILD_OPTS="--build-arg KVMD_VERSION=$(_KVMD_VERSION) --build-arg NEW_SSH_KEYGEN=$(shell uuidgen) --build-arg NEW_HTTPS_CERT=$(shell uuidgen)" \
		PROJECT=pi-kvm \
		PLATFORM=rpi-2 \
		STAGES="__init__ os watchdog ro pikvm rootssh __cleanup__" \
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
