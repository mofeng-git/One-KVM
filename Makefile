-include testenv/config.mk

TESTENV_IMAGE ?= kvmd-testenv
TESTENV_HID ?= /dev/ttyS10
TESTENV_VIDEO ?= /dev/video0
TESTENV_LOOP ?= /dev/loop7

USTREAMER_VERSION = $(shell curl --silent "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h=ustreamer" \
    | grep "^pkgver=" \
    | grep -Po "\d+\.\d+[^\"']*")


# =====
all:
	@ echo "Useful commands:"
	@ echo "    make                  # Print this help"
	@ echo "    make textenv          # Build test environment"
	@ echo "    make tox              # Run tests and linters"
	@ echo "    make tox E=pytest     # Run selected test environment"
	@ echo "    make run              # Run kvmd"
	@ echo "    make run CMD=...      # Run specified command inside kvmd environment"
	@ echo "    make run-ipmi         # Run kvmd-ipmi"
	@ echo "    make run-ipmi CMD=... # Run specified command inside kvmd-ipmi environment"
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
			$(if $(NC),--no-cache,) \
			--rm \
			--tag $(TESTENV_IMAGE) \
			--build-arg USTREAMER_VERSION=$(USTREAMER_VERSION) \
		-f testenv/Dockerfile .


tox: testenv
	time docker run --rm \
			--volume `pwd`:/src:ro \
			--volume `pwd`/testenv:/src/testenv:rw \
			--volume `pwd`/testenv/tests:/src/testenv/tests:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
		-it $(TESTENV_IMAGE) bash -c " \
			cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /src/testenv/main.yaml /etc/kvmd \
			&& cd /src \
			&& tox -c testenv/tox.ini $(if $(E),-e $(E),-p auto) \
		"


run: testenv
	sudo modprobe loop
	- docker run --rm --name kvmd \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/web:/usr/share/kvmd/web:ro \
			--volume `pwd`/extras:/usr/share/kvmd/extras:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--device $(TESTENV_LOOP):/dev/kvmd-msd \
			--device $(TESTENV_VIDEO):$(TESTENV_VIDEO) \
			--publish 8080:80/tcp \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			(socat PTY,link=$(TESTENV_HID) PTY,link=/dev/ttyS11 &) \
			&& cp -r /usr/share/kvmd/configs.default/nginx/* /etc/kvmd/nginx \
			&& cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /testenv/main.yaml /etc/kvmd \
			&& nginx -c /etc/kvmd/nginx/nginx.conf -g 'user http; error_log stderr;' \
			&& ln -s $(TESTENV_VIDEO) /dev/kvmd-video \
			&& (losetup -d /dev/kvmd-msd || true) \
			&& losetup /dev/kvmd-msd /root/loop.img \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.kvmd) \
		"
	- docker run --rm --device=$(TESTENV_LOOP):/dev/kvmd-msd -it $(TESTENV_IMAGE) losetup -d /dev/kvmd-msd


run-ipmi: testenv
	- docker run --rm --name kvmd-ipmi \
			--volume `pwd`/testenv/run:/run/kvmd:rw \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs.default:ro \
			--publish 6230:623/udp \
		-it $(TESTENV_IMAGE) /bin/bash -c " \
			cp /usr/share/kvmd/configs.default/kvmd/*.yaml /etc/kvmd \
			&& cp /usr/share/kvmd/configs.default/kvmd/*passwd /etc/kvmd \
			&& cp /testenv/main.yaml /etc/kvmd \
			&& $(if $(CMD),$(CMD),python -m kvmd.apps.ipmi) \
		"


regen: testenv
	for file in kvmd/data/keymap.yaml hid/src/keymap.h; do \
		docker run --user `id -u`:`id -g` --rm \
			--volume `pwd`:/src \
		-it $(TESTENV_IMAGE) bash -c "cd src && ./genmap.py keymap.in $$file.mako $$file"; \
	done


release:
	make clean
	make tox
	make clean
	make push
	make bump
	make push
	make clean


bump:
	bumpversion $(if $(V),$(V),minor)


push:
	git push
	git push --tags


clean:
	rm -rf testenv/run/*.{pid,sock} build site dist pkg src v*.tar.gz *.pkg.tar.xz *.egg-info kvmd-*.tar.gz
	find kvmd testenv/tests -name __pycache__ | xargs rm -rf
	make -C hid clean


clean-all: testenv clean
	- docker run --rm \
			--volume `pwd`:/src \
		-it $(TESTENV_IMAGE) bash -c "cd src && rm -rf testenv/{.tox,.mypy_cache,.coverage}"


.PHONY: testenv
