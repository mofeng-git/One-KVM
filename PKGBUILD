# Contributor: Maxim Devaev <mdevaev@gmail.com>
# Author: Maxim Devaev <mdevaev@gmail.com>


pkgname=kvmd
pkgver=0.119
pkgrel=1
pkgdesc="The main Pi-KVM daemon"
url="https://github.com/pi-kvm/kvmd"
license=(GPL)
arch=(any)
depends=(
	python
	python-yaml
	python-aiohttp
	python-aiofiles
	python-passlib
	python-pyudev
	python-raspberry-gpio
	python-pyserial
	python-setproctitle
	python-systemd
	python-dbus
	v4l-utils
)
makedepends=(python-setuptools)
source=("$url/archive/v$pkgver.tar.gz")
md5sums=(SKIP)
install=$pkgname.install


build() {
	cd $srcdir
	rm -rf $pkgname-build
	cp -r kvmd-$pkgver $pkgname-build
	cd $pkgname-build
	python setup.py build
}

package() {
	cd $srcdir/$pkgname-build
	python setup.py install --root="$pkgdir"

	mkdir -p "$pkgdir/usr/lib/systemd/system"
	cp configs/systemd/*.service "$pkgdir/usr/lib/systemd/system"

	mkdir -p "$pkgdir/usr/share/kvmd"
	cp -r web "$pkgdir/usr/share/kvmd"
	cp -r extras "$pkgdir/usr/share/kvmd"
	cp -r configs "$pkgdir/usr/share/kvmd/configs.default"
	rm -rf "$pkgdir/usr/share/kvmd/configs.default/systemd"
	sed -i -e "s/^#PROD//g" "$pkgdir/usr/share/kvmd/configs.default/nginx/nginx.conf"
	find "$pkgdir" -name ".gitignore" -delete
	find "$pkgdir/usr/share/kvmd/configs.default" -type f -exec chmod 444 '{}' \;
	chmod 440 "$pkgdir/usr/share/kvmd/configs.default/kvmd/htpasswd"

	mkdir -p "$pkgdir/etc/kvmd"
}
