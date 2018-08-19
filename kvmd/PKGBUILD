# Contributor: Maxim Devaev <mdevaev@gmail.com>
# Author: Maxim Devaev <mdevaev@gmail.com>


pkgname="kvmd"
pkgver="0.56"
pkgrel="1"
pkgdesc="The main Pi-KVM daemon"
arch=("any")
url="https://github.com/pi-kvm/pi-kvm"
license=("GPL")
depends=(
	"python"
	"python-yaml"
	"python-aiohttp"
	"python-aiofiles"
	"python-pyudev"
	"python-raspberry-gpio"
	"python-pyserial"
)
makedepends=("python-setuptools")
source=("$url/archive/v$pkgver.tar.gz")
md5sums=("SKIP")


build() {
	cd $srcdir
	rm -rf $pkgname-build
	cp -r pi-kvm-$pkgver/kvmd $pkgname-build
	cd $pkgname-build
	python setup.py build
}

package() {
	cd $srcdir/$pkgname-build
	python setup.py install --root=$pkgdir
	install -Dm644 configs/kvmd.service "$pkgdir"/usr/lib/systemd/system/kvmd.service
	mkdir -p "$pkgdir"/usr/share/kvmd
	cp -r web "$pkgdir"/usr/share/kvmd
	cp -r configs "$pkgdir"/usr/share/kvmd
}
