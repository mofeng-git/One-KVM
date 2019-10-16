# Contributor: Maxim Devaev <mdevaev@gmail.com>
# Author: Maxim Devaev <mdevaev@gmail.com>


_variants=(v2-hdmi:rpi4)
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
pkgver=1.26
pkgrel=1
pkgdesc="The main Pi-KVM daemon"
url="https://github.com/pikvm/kvmd"
license=(GPL)
arch=(any)
depends=(
	"python>=3.7"
	"python<3.8"
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
	v4l-utils
	nginx-mainline
	openssl
	platformio
	make
	patch
	raspberrypi-io-access
	"ustreamer>=1.9"
)
makedepends=(python-setuptools)
source=("$url/archive/v$pkgver.tar.gz")
md5sums=(SKIP)
backup=(
	etc/kvmd/{override,logging,auth,meta}.yaml
	etc/kvmd/{ht,ipmi}passwd
	etc/kvmd/nginx/{loc-{login,nocache,proxy,websocket},mime-types,ssl,nginx}.conf
)


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

	install -Dm644 -t "$pkgdir/usr/lib/systemd/system" configs/os/services/*.service
	install -DTm644 configs/os/sysusers.conf "$pkgdir/usr/lib/sysusers.d/kvmd.conf"
	install -DTm644 configs/os/tmpfiles.conf "$pkgdir/usr/lib/tmpfiles.d/kvmd.conf"

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
	install -Dm644 -t "$pkgdir/etc/kvmd/nginx" "$_cfg_default/nginx"/*.conf

	install -Dm644 -t "$pkgdir/etc/kvmd" "$_cfg_default/kvmd"/*.yaml
	install -Dm600 -t "$pkgdir/etc/kvmd" "$_cfg_default/kvmd"/*passwd
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
		backup=(
			etc/sysctl.d/99-kvmd.conf
			etc/udev/rules.d/99-kvmd.rules
			etc/kvmd/main.yaml
		)

		cd \"kvmd-\$pkgver\"

		install -DTm644 configs/os/sysctl.conf \"\$pkgdir/etc/sysctl.d/99-kvmd.conf\"
		install -DTm644 configs/os/udev/$_platform-$_board.rules \"\$pkgdir/etc/udev/rules.d/99-kvmd.rules\"

		if [ -f configs/os/modules-load/$_platform.conf ]; then
			backup=(\"\${backup[@]}\" etc/modules-load.d/kvmd.conf)
			install -DTm644 configs/os/modules-load/$_platform.conf \"\$pkgdir/etc/modules-load.d/kvmd.conf\"
		fi

		if [[ $_platform =~ ^.*-hdmi$ ]]; then
			backup=(\"\${backup[@]}\" etc/kvmd/tc358743-edid.hex)
			install -DTm644 configs/kvmd/tc358743-edid.hex \"\$pkgdir/etc/kvmd/tc358743-edid.hex\"
		fi

		install -DTm444 configs/kvmd/main/$_platform.yaml \"\$pkgdir/etc/kvmd/main.yaml\"
	}"
done
