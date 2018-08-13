BUILD_OPTS ?=

HOSTNAME ?= pikvm
LOCALE ?= en_US.UTF-8
TIMEZONE ?= Europe/Moscow

WEBUI_ADMIN_PASSWD ?= admin


# =====
_BUILD_DIR = ./.build
_KVMD_VERSION = $(shell bash -c 'source ../kvmd/PKGBUILD; echo $$pkgver')


# =====
all:
	@ cat Makefile


v1:
	make _pikvm PIKVM_PLATFORM=v1 PI_BOARD=rpi-2


shell:
	cd $(_BUILD_DIR) && make shell


_pikvm: $(_BUILD_DIR)
	rm -rf $(_BUILD_DIR)/stages/pikvm
	rm -rf $(_BUILD_DIR)/builder/scripts/pikvm
	cp -a platforms/$(PIKVM_PLATFORM) $(_BUILD_DIR)/stages/pikvm
	cd $(_BUILD_DIR) && make binfmt && make _rpi \
		BUILD_OPTS=" $(BUILD_OPTS) \
			--build-arg KVMD_VERSION=$(_KVMD_VERSION) \
			--build-arg NEW_SSH_KEYGEN=$(shell uuidgen) \
			--build-arg WEBUI_ADMIN_PASSWD='$(WEBUI_ADMIN_PASSWD)' \
			--build-arg NEW_HTTPS_CERT=$(shell uuidgen) \
		" \
		PROJECT=pikvm \
		BOARD=$(PI_BOARD) \
		STAGES="__init__ os watchdog ro pikvm rootssh __cleanup__" \
		LOCALE=$(LOCALE) \
		TIMEZONE=$(TIMEZONE)


$(_BUILD_DIR):
	git clone --depth=1 https://github.com/mdevaev/pi-builder $(_BUILD_DIR)


install: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make install HOSTNAME=$(HOSTNAME)


scan: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make scan


clean: $(_BUILD_DIR)
	cd $(_BUILD_DIR) && make clean


clean-all:
	- cd $(_BUILD_DIR) && make clean-all
	rm -rf $(_BUILD_DIR)
