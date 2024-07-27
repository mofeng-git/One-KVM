-include config.mk

TESTENV_IMAGE ?= kvmd-testenv
TESTENV_HID ?= /dev/ttyS10
TESTENV_VIDEO ?= /dev/video0
TESTENV_GPIO ?= /dev/gpiochip0
TESTENV_RELAY ?= $(if $(shell ls /dev/hidraw0 2>/dev/null || true),/dev/hidraw0,)

LIBGPIOD_VERSION ?= 1.6.3

USTREAMER_MIN_VERSION ?= $(shell grep -o 'ustreamer>=[^"]\+' PKGBUILD | sed 's/ustreamer>=//g')

DEFAULT_PLATFORM ?= v2-hdmi-rpi4

DOCKER ?= docker


# =====
define optbool
$(filter $(shell echo $(1) | tr A-Z a-z),yes on 1)
endef


# =====
all:
	@ echo "Useful commands:"
	@ echo "    make                  # Print this help"
	@ echo "    make testenv          # Build test environment"
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
	$(DOCKER) build \
			$(if $(call optbool,$(NC)),--no-cache,) \
			--rm \
			--tag $(TESTENV_IMAGE) \
			--build-arg LIBGPIOD_VERSION=$(LIBGPIOD_VERSION) \
			--build-arg USTREAMER_MIN_VERSION=$(USTREAMER_MIN_VERSION) \
		-f testenv/Dockerfile .
	test -d testenv/.ssl || $(DOCKER) run --rm \
			--volume `pwd`:/src:ro \
			--volume `pwd`/testenv:/src/testenv:rw \
		-t $(TESTENV_IMAGE) bash -c " \
			groupadd kvmd-nginx \
			&& groupadd kvmd-vnc \
			&& /src/scripts/kvmd-gencert --do-the-thing \
			&& /src/scripts/kvmd-gencert --do-the-thing --vnc \
			&& chown -R root:root /etc/kvmd/{nginx,vnc}/ssl \
			&& chmod 664 /etc/kvmd/{nginx,vnc}/ssl/* \
			&& chmod 775 /etc/kvmd/{nginx,vnc}/ssl \
			&& mkdir /src/testenv/.ssl \
			&& mv /etc/kvmd/nginx/ssl /src/testenv/.ssl/nginx \
			&& mv /etc/kvmd/vnc/ssl /src/testenv/.ssl/vnc \
		"


tox: testenv
	time $(DOCKER) run --rm \
			--volume `pwd`:/src:ro \
			--volume `pwd`/testenv:/src/testenv:rw \
			--volume `pwd`/testenv/tests:/src/testenv/tests:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
		-t $(TESTENV_IMAGE) bash -c " \
			cp -a /src/testenv/.ssl/nginx /etc/kvmd/nginx/ssl \
			&& cp -a /src/testenv/.ssl/vnc /etc/kvmd/vnc/ssl \
			&& cp /src/testenv/platform /usr/share/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.secret /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& mkdir -p /etc/kvmd/override.d \
			&& cp /src/testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& cd /src \
			&& $(if $(CMD),$(CMD),tox -q -c testenv/tox.ini $(if $(E),-e $(E),-p auto)) \
		"


$(TESTENV_GPIO):
	test ! -e $(TESTENV_GPIO)
	sudo modprobe gpio-mockup gpio_mockup_ranges=0,40
	test -c $(TESTENV_GPIO)


run: testenv $(TESTENV_GPIO)
	- $(DOCKER) run --rm --name kvmd \
			--privileged \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/testenv/env.py:/kvmd/env.py:ro \
			--volume `pwd`/web:/usr/share/kvmd/web:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
			--device $(TESTENV_VIDEO):$(TESTENV_VIDEO) \
			--device $(TESTENV_GPIO):$(TESTENV_GPIO) \
			$(if $(TESTENV_RELAY),--device $(TESTENV_RELAY):$(TESTENV_RELAY),) \
			--publish 8080:8080/tcp \
			--publish 4430:4430/tcp \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			mkdir -p /tmp/kvmd-nginx \
			&& mount -t debugfs none /sys/kernel/debug \
			&& test -d /sys/kernel/debug/gpio-mockup/`basename $(TESTENV_GPIO)`/ || (echo \"Missing GPIO mockup\" && exit 1) \
			&& (socat PTY,link=$(TESTENV_HID) PTY,link=/dev/ttyS11 &) \
			&& cp -r /usr/share/kvmd/configs.default/nginx/* /etc/kvmd/nginx \
			&& cp -a /testenv/.ssl/nginx /etc/kvmd/nginx/ssl \
			&& cp -a /testenv/.ssl/vnc /etc/kvmd/vnc/ssl \
			&& cp /testenv/platform /usr/share/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.secret /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& ln -s /testenv/web.css /etc/kvmd/web.css \
			&& mkdir -p /etc/kvmd/override.d \
			&& cp /testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf \
			&& nginx -c /etc/kvmd/nginx/nginx.conf -g 'user http; error_log stderr;' \
			&& ln -s $(TESTENV_VIDEO) /dev/kvmd-video \
			&& ln -s $(TESTENV_GPIO) /dev/kvmd-gpio \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.kvmd --run) \
		"


run-cfg: testenv
	- $(DOCKER) run --rm --name kvmd-cfg \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			cp -a /testenv/.ssl/nginx /etc/kvmd/nginx/ssl \
			&& cp -a /testenv/.ssl/vnc /etc/kvmd/vnc/ssl \
			&& cp /testenv/platform /usr/share/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.secret /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& mkdir -p /etc/kvmd/override.d \
			&& cp /testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.kvmd -m) \
		"


run-ipmi: testenv
	- $(DOCKER) run --rm --name kvmd-ipmi \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
			--publish 6230:623/udp \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			cp -a /testenv/.ssl/nginx /etc/kvmd/nginx/ssl \
			&& cp -a /testenv/.ssl/vnc /etc/kvmd/vnc/ssl \
			&& cp /testenv/platform /usr/share/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.secret /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& mkdir -p /etc/kvmd/override.d \
			&& cp /testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.ipmi --run) \
		"


run-vnc: testenv
	- $(DOCKER) run --rm --name kvmd-vnc \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--volume `pwd`/contrib/keymaps:/usr/share/kvmd/keymaps:ro \
			--publish 5900:5900/tcp \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			cp -a /testenv/.ssl/nginx /etc/kvmd/nginx/ssl \
			&& cp -a /testenv/.ssl/vnc /etc/kvmd/vnc/ssl \
			&& cp /testenv/platform /usr/share/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.secret /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/main/$(if $(P),$(P),$(DEFAULT_PLATFORM)).yaml /etc/kvmd/main.yaml \
			&& mkdir -p /etc/kvmd/override.d \
			&& cp /testenv/$(if $(P),$(P),$(DEFAULT_PLATFORM)).override.yaml /etc/kvmd/override.yaml \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.vnc --run) \
		"


regen: keymap pug


keymap: testenv
	$(DOCKER) run --user `id -u`:`id -g` --rm \
		--volume `pwd`:/src \
	-it $(TESTENV_IMAGE) bash -c "cd src \
		&& ./genmap.py keymap.csv kvmd/keyboard/mappings.py.mako kvmd/keyboard/mappings.py \
		&& ./genmap.py keymap.csv hid/arduino/lib/drivers/usb-keymap.h.mako hid/arduino/lib/drivers/usb-keymap.h \
		&& ./genmap.py keymap.csv hid/arduino/lib/drivers-avr/ps2/keymap.h.mako hid/arduino/lib/drivers-avr/ps2/keymap.h \
		&& ./genmap.py keymap.csv hid/pico/src/ph_usb_keymap.h.mako hid/pico/src/ph_usb_keymap.h \
	"


pug: testenv
	$(DOCKER) run --user `id -u`:`id -g` --rm \
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
	make -C hid/arduino clean
	make -C hid/pico clean


clean-all: testenv clean
	make -C hid/arduino clean-all
	make -C hid/pico clean-all
	- $(DOCKER) run --rm \
			--volume `pwd`:/src \
		-it $(TESTENV_IMAGE) bash -c "cd src && rm -rf testenv/{.ssl,.tox,.mypy_cache,.coverage}"


.PHONY: testenv
