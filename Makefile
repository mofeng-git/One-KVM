TESTENV_IMAGE ?= kvmd-testenv
TESTENV_HID ?= /dev/ttyS10
TESTENV_VIDEO ?= /dev/video0
TESTENV_LOOP ?= /dev/loop7
TESTENV_CMD ?= /bin/bash -c " \
		(socat PTY,link=$(TESTENV_HID) PTY,link=/dev/ttyS11 &) \
		&& rm -rf /etc/nginx/* \
		&& cp -r /configs/nginx/* /etc/nginx \
		&& nginx -c /etc/nginx/nginx.conf \
		&& ln -s $(TESTENV_VIDEO) /dev/kvmd-streamer \
		&& (losetup -d /dev/kvmd-msd || true) \
		&& losetup /dev/kvmd-msd /root/loop.img \
		&& python -m kvmd -c testenv/kvmd.conf \
	"


# =====
all:
	cat Makefile


run:
	sudo modprobe loop
	docker build $(TESTENV_OPTS) --rm --tag $(TESTENV_IMAGE) -f testenv/Dockerfile .
	- docker run --rm \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/web:/usr/share/kvmd/web:ro \
			--volume `pwd`/testenv:/testenv:ro \
			--volume `pwd`/configs:/configs:ro \
			--device $(TESTENV_LOOP):/dev/kvmd-msd \
			--device $(TESTENV_VIDEO):$(TESTENV_VIDEO) \
			--publish 8080:80/tcp \
			--publish 8081:8081/tcp \
			--publish 8082:8082/tcp \
		-it $(TESTENV_IMAGE) $(TESTENV_CMD)
	- docker run --rm --device=$(TESTENV_LOOP):/dev/kvmd-msd -it $(TESTENV_IMAGE) losetup -d /dev/kvmd-msd


shell:
	make run TESTENV_CMD=/bin/bash


regen:
	python3 genmap.py


release:
	make clean
	#make tox
	#make clean
	make push
	make bump
	make push
	make clean


tox:
	tox


bump:
	bumpversion minor


push:
	git push
	git push --tags


clean:
	rm -rf build site dist pkg src *.egg-info kvmd-*.tar.gz
	find -name __pycache__ | xargs rm -rf
	make -C hid clean


clean-all: clean
	rm -rf .tox .mypy_cache
