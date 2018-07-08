TESTENV_IMAGE ?= kvmd-testenv
TESTENV_VIDEO ?= /dev/video0
TESTENV_LOOP ?= /dev/loop7
TESTENV_CMD ?= /bin/bash -c " \
		ln -s $(TESTENV_VIDEO) /dev/kvmd-streamer \
		&& (losetup -d /dev/kvmd-msd || true) \
		&& losetup /dev/kvmd-msd /root/loop.img \
		&& python -m kvmd -c testenv/kvmd.yaml \
	"


# =====
all:
	cat Makefile


run:
	docker build --rm --tag $(TESTENV_IMAGE) -f testenv/Dockerfile .
	- docker run --rm \
			--volume `pwd`/kvmd:/kvmd:ro \
			--volume `pwd`/web:/web:ro \
			--volume `pwd`/testenv:/testenv:ro \
			--device $(TESTENV_LOOP):/dev/kvmd-msd \
			--device $(TESTENV_VIDEO):$(TESTENV_VIDEO) \
			--publish 8080:8080/tcp \
			--publish 8081:8081/tcp \
			--publish 8082:8082/tcp \
		-it $(TESTENV_IMAGE) $(TESTENV_CMD)
	- docker run --rm --device=$(TESTENV_LOOP):/dev/kvmd-msd -it $(TESTENV_IMAGE) losetup -d /dev/kvmd-msd


shell:
	make run TESTENV_CMD=/bin/bash


release:
	make clean
	#make tox
	make clean
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


clean-all: clean
	rm -rf .tox .mypy_cache
