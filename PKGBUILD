# Contributor: Maxim Devaev <mdevaev@gmail.com>
# Author: Maxim Devaev <mdevaev@gmail.com>


pkgname=kvmd
pkgver=0.96
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
	python-pyudev
	python-raspberry-gpio
	python-pyserial
	python-setproctitle
	python-systemd
)
makedepends=(python-setuptools)
source=("$url/archive/v$pkgver.tar.gz")
md5sums=(SKIP)
install=kvmd.install


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
	install -Dm644 configs/systemd/kvmd.service "$pkgdir/usr/lib/systemd/system/kvmd.service"
	install -Dm644 configs/systemd/kvmd-tc358743.service "$pkgdir/usr/lib/systemd/system/kvmd-tc358743.service"
	mkdir -p "$pkgdir/usr/share/kvmd"
	mkdir -p "$pkgdir/etc/kvmd"
	cp -r web "$pkgdir/usr/share/kvmd"
	cp -r configs "$pkgdir/usr/share/kvmd"
	find "$pkgdir" -name ".gitignore" -delete
}
