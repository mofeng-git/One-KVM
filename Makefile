-include testenv/config.mk

TESTENV_IMAGE ?= kvmd-testenv
TESTENV_HID ?= /dev/ttyS10
TESTENV_VIDEO ?= /dev/video0
TESTENV_GPIO ?= /dev/gpiochip0
TESTENV_RELAY ?= $(if $(shell ls /dev/hidraw0 2>/dev/null || true),/dev/hidraw0,)

LIBGPIOD_VERSION ?= 1.5.2

USTREAMER_MIN_VERSION ?= $(shell grep -o 'ustreamer>=[^"]\+' PKGBUILD | sed 's/ustreamer>=//g')

DEFAULT_PLATFORM ?= v2-hdmi-rpi4


# =====
define optbool
$(filter $(shell echo $(1) | tr A-Z a-z),yes on 1)
endef


# =====
all:
	@ echo "Useful commands:"
	@ echo "    make                  # Print this help"
	@ echo "    make textenv          # Build test environment"
	@ echo "    make tox              # Run tests and linters"
	@ echo "    make tox E=pytest     # Run selected test environment"
	@ echo "    make gpio             # Create gpio mockup"
	@ echo "    make run              # Run kvmd"
	@ echo "    make run CMD=...      # Run specified command inside kvmd environment"
	@ echo "    make run-cfg          # Run kvmd -m"
	@ echo "    make run-ipmi         # Run kvmd-ipmi"
	@ echo "    make run-ipmi CMD=... # Run specified command inside kvmd-ipmi environment"
	@ echo "    make run-vnc          # Run kvmd-vnc"
	@ echo "    make run-vnc  CMD=... # Run specified command inside kvmd-vnc environment"
	@ echo "    make regen            # Regen some sources like keymap"
	@ echo "    make bump             # Bump minor version"
	@ echo "    make bump V=major     # Bump major version"
	@ echo "    make release          # Publish the new release (include bump minor)"
	@ echo "    make clean            # Remove garbage"
	@ echo "    make clean-all        # Remove garbage and test results"
	@ echo
	@ echo "Also you can add option NC=1 to rebuild docker test environment"


testenv:
	docker build \
			$(if $(call optbool,$(NC)),--no-cache,) \
			--rm \
			--tag $(TESTENV_IMAGE) \
			--build-arg LIBGPIOD_VERSION=$(LIBGPIOD_VERSION) \
			--build-arg USTREAMER_MIN_VERSION=$(USTREAMER_MIN_VERSION) \
		-f testenv/Dockerfile .


tox: testenv
	time docker run --rm \
			--volume `pwd`:/src:ro \
			--volume `pwd`/testenv:/src/testenv:rw \
			--volume `pwd`/testenv/tests:/src/testenv/tests:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
		-t $(TESTENV_IMAGE) bash -c " \
			cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& cp /src/testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& cd /src \
			&& tox -q -c testenv/tox.ini $(if $(E),-e $(E),-p auto) \
		"


$(TESTENV_GPIO):
	test ! -e $(TESTENV_GPIO)
	sudo modprobe gpio-mockup gpio_mockup_ranges=0,40
	test -c $(TESTENV_GPIO)


run: testenv $(TESTENV_GPIO)
	- docker run --rm --name kvmd \
			--cap-add SYS_ADMIN \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/web:/usr/share/kvmd/web:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
			--device $(TESTENV_VIDEO):$(TESTENV_VIDEO) \
			--device $(TESTENV_GPIO):$(TESTENV_GPIO) \
			--env KVMD_GPIO_DEVICE_PATH=$(TESTENV_GPIO) \
			--env KVMD_SYSFS_PREFIX=/fake_sysfs \
			--env KVMD_PROCFS_PREFIX=/fake_procfs \
			$(if $(TESTENV_RELAY),--device $(TESTENV_RELAY):$(TESTENV_RELAY),) \
			--publish 8080:80/tcp \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			mount -t debugfs none /sys/kernel/debug \
			&& test -d /sys/kernel/debug/gpio-mockup/`basename $(TESTENV_GPIO)`/ \
			&& (socat PTY,link=$(TESTENV_HID) PTY,link=/dev/ttyS11 &) \
			&& cp -r /usr/share/kvmd/configs.default/nginx/* /etc/kvmd/nginx \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& cp /testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& nginx -c /etc/kvmd/nginx/nginx.conf -g 'user http; error_log stderr;' \
			&& ln -s $(TESTENV_VIDEO) /dev/kvmd-video \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.kvmd) \
		"


run-cfg: testenv
	- docker run --rm --name kvmd-cfg \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& cp /testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.kvmd -m) \
		"


run-ipmi: testenv
	- docker run --rm --name kvmd-ipmi \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
			--publish 6230:623/udp \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& cp /testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.ipmi) \
		"


run-vnc: testenv
	- docker run --rm --name kvmd-vnc \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
			--publish 5900:5900/tcp \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& cp /testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.vnc) \
		"


regen: keymap pug


keymap: testenv
	docker run --user `id -u`:`id -g` --rm \
		--volume `pwd`:/src \
	-it $(TESTENV_IMAGE) bash -c "cd src \
		&& ./genmap.py keymap.csv kvmd/keyboard/mappings.py.mako kvmd/keyboard/mappings.py \
		&& ./genmap.py keymap.csv hid/src/usb/keymap.h.mako hid/src/usb/keymap.h \
		&& ./genmap.py keymap.csv hid/src/ps2/keymap.h.mako hid/src/ps2/keymap.h \
	"


pug: testenv
	docker run --user `id -u`:`id -g` --rm \
		--volume `pwd`:/src \
	-it $(TESTENV_IMAGE) bash -c "cd src \
		&& pug --pretty web/index.pug -o web \
		&& pug --pretty web/login/index.pug -o web/login \
		&& pug --pretty web/kvm/index.pug -o web/kvm \
		&& pug --pretty web/ipmi/index.pug -o web/ipmi \
		&& pug --pretty web/vnc/index.pug -o web/vnc \
	"


release:
	make clean
	make tox
	make clean
	make push
	make bump V=$(V)
	make push
	make clean


bump:
	bumpversion $(if $(V),$(V),minor)


push:
	git push
	git push --tags


clean:
	rm -rf testenv/run/*.{pid,sock} build site dist pkg src v*.tar.gz *.pkg.tar.{xz,zst} *.egg-info kvmd-*.tar.gz
	find kvmd testenv/tests -name __pycache__ | xargs rm -rf
	make -C hid clean


clean-all: testenv clean
	- docker run --rm \
			--volume `pwd`:/src \
		-it $(TESTENV_IMAGE) bash -c "cd src && rm -rf testenv/{.tox,.mypy_cache,.coverage}"


.PHONY: testenv
