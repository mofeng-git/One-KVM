all:
	cat Makefile

bencoder:
	python3 setup.py build_ext --inplace

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
