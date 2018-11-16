TESTENV_IMAGE ?= kvmd-testenv
TESTENV_HID ?= /dev/ttyS10
TESTENV_VIDEO ?= /dev/video0
TESTENV_LOOP ?= /dev/loop7
TESTENV_CMD ?= /bin/bash -c " \
		(socat PTY,link=$(TESTENV_HID) PTY,link=/dev/ttyS11 &) \
		&& rm -rf /etc/nginx/* \
		&& cp -r /usr/share/kvmd/configs/nginx/* /etc/nginx \
		&& mkdir -p /etc/kvmd \
		&& cp /usr/share/kvmd/configs/kvmd/{meta.yaml,logging.yaml} /etc/kvmd \
		&& cp /testenv/kvmd.yaml /etc/kvmd \
		&& nginx -c /etc/nginx/nginx.conf \
		&& ln -s $(TESTENV_VIDEO) /dev/kvmd-streamer \
		&& (losetup -d /dev/kvmd-msd || true) \
		&& losetup /dev/kvmd-msd /root/loop.img \
		&& python -m kvmd.apps.kvmd -c /etc/kvmd/kvmd.yaml \
	"


# =====
all:
	cat Makefile


tox: _testenv
	- docker run --rm \
			--volume `pwd`:/kvmd \
		-it $(TESTENV_IMAGE) bash -c "cd kvmd && tox -c testenv/tox.ini"


run: _testenv
	sudo modprobe loop
	- docker run --rm \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/web:/usr/share/kvmd/web:ro \
			--volume `pwd`/configs:/usr/share/kvmd/configs:ro \
			--volume `pwd`/testenv:/testenv:ro \
			--device $(TESTENV_LOOP):/dev/kvmd-msd \
			--device $(TESTENV_VIDEO):$(TESTENV_VIDEO) \
			--publish 8080:80/tcp \
			--publish 8081:8081/tcp \
			--publish 8082:8082/tcp \
		-it $(TESTENV_IMAGE) $(TESTENV_CMD)
	- docker run --rm --device=$(TESTENV_LOOP):/dev/kvmd-msd -it $(TESTENV_IMAGE) losetup -d /dev/kvmd-msd


run-no-cache:
	make run TESTENV_OPTS=--no-cache


shell:
	make run TESTENV_CMD=/bin/bash


regen:
	python3 genmap.py


release:
	make clean
	make tox
	make clean
	make push
	make bump
	make push
	make clean

bump:
	bumpversion minor


push:
	git push
	git push --tags


clean:
	rm -rf build site dist pkg src *.egg-info kvmd-*.tar.gz
	find kvmd -name __pycache__ | xargs rm -rf
	rm -rf __pycache__
	make -C hid clean


clean-all: _testenv clean
	- docker run --rm \
			--volume `pwd`:/kvmd \
		-it $(TESTENV_IMAGE) bash -c "cd kvmd && rm -rf testenv/{.tox,.mypy_cache}"


_testenv:
	docker build $(TESTENV_OPTS) --rm --tag $(TESTENV_IMAGE) -f testenv/Dockerfile .
