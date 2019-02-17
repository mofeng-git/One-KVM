# Contributor: Maxim Devaev <mdevaev@gmail.com>
# Author: Maxim Devaev <mdevaev@gmail.com>


_PLATFORMS="v1-vga v1-hdmi"
_BOARDS="rpi2 rpi3"


pkgname=(kvmd)
for _platform in $_PLATFORMS; do
	for _board in $_BOARDS; do
		pkgname+=(kvmd-platform-$_platform-$_board)
	done
done
pkgbase=kvmd
pkgver=0.129
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
	python-pygments
	v4l-utils
	nginx
)
makedepends=(python-setuptools)
source=("$url/archive/v$pkgver.tar.gz")
md5sums=(SKIP)


build() {
	cd "$srcdir"
	rm -rf $pkgname-build
	cp -r kvmd-$pkgver $pkgname-build
	cd $pkgname-build
	python setup.py build
}

package_kvmd() {
	install=$pkgname.install

	cd "$srcdir/$pkgname-build"
	python setup.py install --root="$pkgdir"

	mkdir -p "$pkgdir/usr/lib/systemd/system"
	cp configs/os/systemd/*.service "$pkgdir/usr/lib/systemd/system"

	_cfgdir="$pkgdir/usr/share/kvmd/configs.default"
	mkdir -p "$pkgdir/usr/share/kvmd"
	cp -r web "$pkgdir/usr/share/kvmd"
	cp -r extras "$pkgdir/usr/share/kvmd"
	cp -r configs "$_cfgdir"

	rm -rf "$_cfgdir/os/systemd"
	find "$pkgdir" -name ".gitignore" -delete
	sed -i -e "s/^#PROD//g" "$_cfgdir/nginx/nginx.conf"
	find "$_cfgdir" -type f -exec chmod 444 '{}' \;
	chmod 440 "$_cfgdir/kvmd/htpasswd"

	mkdir -p "$pkgdir/etc/kvmd/nginx"
	for path in "$_cfgdir/nginx/*.conf"; do
		ln -sf "/usr/share/kvmd/configs.default/nginx/`basename $path`" "$pkgdir/etc/kvmd/nginx"
	done
}

export pkgdir
for _platform in $_PLATFORMS; do
	for _board in $_BOARDS; do
		eval "package_kvmd-platform-$_platform-$_board() {
			mkdir -p \"$pkgdir/etc/\"{sysctl.d,udev/rules.d,modules-load.d}
			_osdir=\"/usr/share/kvmd/configs.default/os\"
			ln -sf \"$_osdir/sysctl.conf\" \"$pkgdir/etc/sysctl.d/99-pikvm.conf\"
			ln -sf \"$_osdir/udev/$_platform-$_board.rules\" \"$pkgdir/etc/udev/rules.d/99-pikvm.rules\"
			ln -sf \"$_osdir/modules-load/$_platform.conf\" \"$pkgdir/etc/modules-load.d/pikvm.conf\"
		}"
	done
done
