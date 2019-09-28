# Contributor: Maxim Devaev <mdevaev@gmail.com>
# Author: Maxim Devaev <mdevaev@gmail.com>


_variants=(v2-hdmi-rpi4)
for _platform in v0-vga v0-hdmi v1-vga v1-hdmi; do
	for _board in rpi2 rpi3; do
		_variants+=($_platform:$_board)
	done
done


pkgname=(kvmd)
for _variant in "${_variants[@]}"; do
	_platform=${_variant%:*}
	_board=${_variant#*:}
	pkgname+=(kvmd-platform-$_platform-$_board)
done
pkgbase=kvmd
pkgver=1.11
pkgrel=1
pkgdesc="The main Pi-KVM daemon"
url="https://github.com/pikvm/kvmd"
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
	python-psutil
	python-systemd
	python-dbus
	python-pygments
	python-pyghmi
	psmisc
	v4l-utils
	nginx-mainline
	openssl
	platformio
	make
	raspberrypi-io-access
	"ustreamer>=1.5"
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
	cp configs/os/services/*.service "$pkgdir/usr/lib/systemd/system"

	mkdir -p "$pkgdir/usr/lib/sysusers.d"
	cp configs/os/sysusers.conf "$pkgdir/usr/lib/sysusers.d/kvmd.conf"

	mkdir -p "$pkgdir/usr/lib/tmpfiles.d"
	cp configs/os/tmpfiles.conf "$pkgdir/usr/lib/tmpfiles.d/kvmd.conf"

	mkdir -p "$pkgdir/usr/share/kvmd"
	cp -r {hid,web,extras} "$pkgdir/usr/share/kvmd"

	local _cfg_default="$pkgdir/usr/share/kvmd/configs.default"
	mkdir -p "$_cfg_default"
	cp -r configs/* "$_cfg_default"

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

for _variant in "${_variants[@]}"; do
	_platform=${_variant%:*}
	_board=${_variant#*:}
	eval "package_kvmd-platform-$_platform-$_board() {
		pkgdesc=\"Pi-KVM platform configs - $_platform for $_board\"
		depends=(kvmd)
		if [[ $_platform =~ ^.*-hdmi$ ]]; then
			depends=(\"\${depends[@]}\" \"tc358743-dkms>=0.3\")
		fi

		mkdir -p \"\$pkgdir/etc\"/{kvmd,sysctl.d,udev/rules.d,modules-load.d}

		local _cfg_default=\"/usr/share/kvmd/configs.default\"

		ln -sf \"\$_cfg_default/os/sysctl.conf\" \"\$pkgdir/etc/sysctl.d/99-kvmd.conf\"
		ln -sf \"\$_cfg_default/os/udev/$_platform-$_board.rules\" \"\$pkgdir/etc/udev/rules.d/99-kvmd.rules\"
		ln -sf \"\$_cfg_default/os/modules-load/$_platform.conf\" \"\$pkgdir/etc/modules-load.d/kvmd.conf\"

		ln -sf \"\$_cfg_default/kvmd/main/$_platform.yaml\" \"\$pkgdir/etc/kvmd/main.yaml\"
		if [[ $_platform =~ ^.*-hdmi$ ]]; then
			ln -sf \"\$_cfg_default/kvmd/tc358743-edid.hex\" \"\$pkgdir/etc/kvmd/tc358743-edid.hex\"
		fi
	}"
done
