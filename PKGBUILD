# Contributor: Maxim Devaev <mdevaev@gmail.com>
# Author: Maxim Devaev <mdevaev@gmail.com>


[ -n "$PIKVM_PLATFORM" ] || PIKVM_PLATFORM="v0-vga v0-hdmi v1-vga v1-hdmi"
[ -n "$PIKVM_BOARD" ] || PIKVM_BOARD="rpi2 rpi3"


pkgname=(kvmd)
for _platform in $PIKVM_PLATFORM; do
	for _board in $PIKVM_BOARD; do
		pkgname+=(kvmd-platform-$_platform-$_board)
	done
done
pkgbase=kvmd
pkgver=0.181
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
	python-raspberry-gpio
	python-pyserial
	python-setproctitle
	python-systemd
	python-dbus
	python-pygments
	python-pyghmi
	psmisc
	v4l-utils
	nginx-mainline
	openssl
	raspberrypi-io-access
)
optdepends=(
	dkms
	tc358743-dkms
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

	mkdir -p "$pkgdir/usr/lib/tmpfiles.d"
	cp configs/os/tmpfiles.conf "$pkgdir/usr/lib/tmpfiles.d/kvmd.conf"

	mkdir -p "$pkgdir/usr/share/kvmd"
	cp -r web "$pkgdir/usr/share/kvmd"
	cp -r extras "$pkgdir/usr/share/kvmd"

	local _cfg_default="$pkgdir/usr/share/kvmd/configs.default"
	mkdir -p "$_cfg_default"
	cp -r configs/* "$_cfg_default"

	rm -rf "$_cfg_default/os/systemd"
	find "$pkgdir" -name ".gitignore" -delete
	sed -i -e "s/^#PROD//g" "$_cfg_default/nginx/nginx.conf"
	find "$_cfg_default" -type f -exec chmod 444 '{}' \;
	chmod 400 "$_cfg_default/kvmd"/*passwd

	mkdir -p "$pkgdir/etc/kvmd/nginx/ssl"
	chmod 750 "$pkgdir/etc/kvmd/nginx/ssl"
	for _path in "$_cfg_default/kvmd"/*.yaml; do
		ln -sf "/usr/share/kvmd/configs.default/kvmd/`basename $_path`" "$pkgdir/etc/kvmd"
	done
	rm "$pkgdir/etc/kvmd"/{override.yaml,logging.yaml,auth.yaml,meta.yaml}
	cp "$_cfg_default/kvmd"/{override.yaml,logging.yaml,auth.yaml,meta.yaml} "$pkgdir/etc/kvmd"
	cp "$_cfg_default/kvmd/"*passwd "$pkgdir/etc/kvmd"
	chmod 600 "$pkgdir/etc/kvmd/"*passwd
	for _path in "$_cfg_default/nginx"/*.conf; do
		ln -sf "/usr/share/kvmd/configs.default/nginx/`basename $_path`" "$pkgdir/etc/kvmd/nginx"
	done
}


for _platform in $PIKVM_PLATFORM; do
	for _board in $PIKVM_BOARD; do
		eval "package_kvmd-platform-$_platform-$_board() {
			pkgdesc=\"Pi-KVM platform configs - $_platform for $_board\"

			mkdir -p \"\$pkgdir/etc\"/{kvmd,sysctl.d,udev/rules.d,modules-load.d}

			local _cfg_default=\"/usr/share/kvmd/configs.default\"

			ln -sf \"\$_cfg_default/os/sysctl.conf\" \"\$pkgdir/etc/sysctl.d/99-pikvm.conf\"
			ln -sf \"\$_cfg_default/os/udev/$_platform-$_board.rules\" \"\$pkgdir/etc/udev/rules.d/99-pikvm.rules\"
			ln -sf \"\$_cfg_default/os/modules-load/$_platform.conf\" \"\$pkgdir/etc/modules-load.d/pikvm.conf\"

			ln -sf \"\$_cfg_default/kvmd/main/$_platform.yaml\" \"\$pkgdir/etc/kvmd/main.yaml\"
			if [ $_platform == v1-hdmi ]; then
				ln -sf \"\$_cfg_default/kvmd/tc358743-edid.hex\" \"\$pkgdir/etc/kvmd/tc358743-edid.hex\"
			fi
		}"
	done
done
