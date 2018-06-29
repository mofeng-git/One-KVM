# Contributor: Maxim Devaev <mdevaev@gmail.com>
# Author: Maxim Devaev <mdevaev@gmail.com>


pkgname="kvmd"
pkgver="0.5"
pkgrel="1"
pkgdesc="The main Pi-KVM daemon"
arch=("any")
url="https://github.com/mdevaev/pi-kvm"
license=("GPL")
depends=(
	"python"
	"python-yaml"
	"python-aiohttp"
	"python-raspberry-gpio"
)
backup=("etc/kvmd.yaml")
makedepends=("python-setuptools" "wget")


build() {
	cd $srcdir
	if [ ! -d pi-kvm-$pkgver ]; then
		msg "Downloading tag v$pkgver..."
		wget $url/archive/v$pkgver.tar.gz
		tar -xzf v$pkgver.tar.gz
	fi

	rm -rf $pkgname-build
	cp -r pi-kvm-$pkgver/kvmd $pkgname-build
	cd $pkgname-build

	python setup.py build
}

package() {
	cd $srcdir/$pkgname-build
	python setup.py install --root=$pkgdir

	install -Dm644 kvmd.yaml $pkgdir/etc/kvmd.yaml
	install -Dm644 kvmd.service "$pkgdir"/usr/lib/systemd/system/kvmd.service
}
