# Contributor: Maxim Devaev <mdevaev@gmail.com>
# Author: Maxim Devaev <mdevaev@gmail.com>


_variants=(
	v0-hdmi:zero2w
	v0-hdmi:rpi2
	v0-hdmi:rpi3

	v0-hdmiusb:zero2w
	v0-hdmiusb:rpi2
	v0-hdmiusb:rpi3

	v1-hdmi:zero2w
	v1-hdmi:rpi2
	v1-hdmi:rpi3

	v1-hdmiusb:zero2w
	v1-hdmiusb:rpi2
	v1-hdmiusb:rpi3

	v2-hdmi:zero2w
	v2-hdmi:rpi3
	v2-hdmi:rpi4

	v2-hdmiusb:rpi4

	v3-hdmi:rpi4

	v4mini-hdmi:rpi4
	v4plus-hdmi:rpi4
)


pkgname=(kvmd)
for _variant in "${_variants[@]}"; do
	_platform=${_variant%:*}
	_board=${_variant#*:}
	pkgname+=(kvmd-platform-$_platform-$_board)
done
pkgbase=kvmd
pkgver=4.2
pkgrel=1
pkgdesc="The main PiKVM daemon"
url="https://github.com/pikvm/kvmd"
license=(GPL)
arch=(any)
depends=(
	"python>=3.12"
	"python<3.13"
	python-yaml
	python-aiohttp
	python-aiofiles
	python-async-lru
	python-passlib
	python-pyotp
	python-qrcode
	python-periphery
	python-pyserial
	python-pyserial-asyncio
	python-spidev
	python-setproctitle
	python-psutil
	python-netifaces
	python-systemd
	python-dbus
	python-dbus-next
	python-pygments
	python-pyghmi
	python-pam
	python-pillow
	python-xlib
	libxkbcommon
	python-hidapi
	python-six
	python-pyrad
	python-ldap
	python-zstandard
	python-mako
	"libgpiod>=2.1"
	freetype2
	"v4l-utils>=1.22.1-1"
	"nginx-mainline>=1.25.1"
	openssl
	sudo
	iptables
	iproute2
	dnsmasq
	ipmitool
	"janus-gateway-pikvm>=0.14.2-3"
	certbot
	platform-io-access
	raspberrypi-utils
	"ustreamer>=6.11"

	# Systemd UDEV bug
	"systemd>=248.3-2"

	# https://bugzilla.redhat.com/show_bug.cgi?id=2035802
	# https://archlinuxarm.org/forum/viewtopic.php?f=15&t=15725&start=40
	"zstd>=1.5.1-2.1"

	# Possible hotfix for the new os update
	openssl-1.1

	# Bootconfig
	dos2unix
	parted
	e2fsprogs
	openssh
	wpa_supplicant
	run-parts

	# fsck for /boot
	dosfstools

	# pgrep for kvmd-udev-restart-pass
	procps-ng

	# Misc
	hostapd
)
optdepends=(
	tesseract
)
conflicts=(
	python-pikvm
	python-aiohttp-pikvm
	platformio
	avrdude-pikvm
)
makedepends=(
	python-setuptools
	python-pip
)
source=("$url/archive/v$pkgver.tar.gz")
md5sums=(SKIP)
backup=(
	etc/kvmd/{override,logging,auth,meta}.yaml
	etc/kvmd/{ht,ipmi,vnc}passwd
	etc/kvmd/totp.secret
	etc/kvmd/nginx/{kvmd.ctx-{http,server},certbot.ctx-server}.conf
	etc/kvmd/nginx/loc-{login,nocache,proxy,websocket,nobuffering,bigpost}.conf
	etc/kvmd/nginx/{mime-types,ssl}.conf
	etc/kvmd/nginx/nginx.conf.mako
	etc/kvmd/janus/janus{,.plugin.ustreamer,.transport.websockets}.jcfg
	etc/kvmd/web.css
)


package_kvmd() {
	install=$pkgname.install

	cd "$srcdir/kvmd-$pkgver"
	pip install --root="$pkgdir" --no-deps .

	install -Dm755 -t "$pkgdir/usr/bin" scripts/kvmd-{bootconfig,gencert,certbot}

	install -Dm644 -t "$pkgdir/usr/lib/systemd/system" configs/os/services/*
	install -DTm644 configs/os/sysusers.conf "$pkgdir/usr/lib/sysusers.d/kvmd.conf"
	install -DTm644 configs/os/tmpfiles.conf "$pkgdir/usr/lib/tmpfiles.d/kvmd.conf"

	mkdir -p "$pkgdir/usr/share/kvmd"
	cp -r {hid,web,extras,contrib/keymaps} "$pkgdir/usr/share/kvmd"
	find "$pkgdir/usr/share/kvmd/web" -name '*.pug' -exec rm -f '{}' \;

	local _cfg_default="$pkgdir/usr/share/kvmd/configs.default"
	mkdir -p "$_cfg_default"
	cp -r configs/* "$_cfg_default"

	find "$pkgdir" -name ".gitignore" -delete
	find "$_cfg_default" -type f -exec chmod 444 '{}' \;
	chmod 400 "$_cfg_default/kvmd"/*passwd
	chmod 400 "$_cfg_default/kvmd"/*.secret
	chmod 750 "$_cfg_default/os/sudoers"
	chmod 400 "$_cfg_default/os/sudoers"/*

	mkdir -p "$pkgdir/etc/kvmd/"{nginx,vnc}"/ssl"
	chmod 755 "$pkgdir/etc/kvmd/"{nginx,vnc}"/ssl"
	install -Dm444 -t "$pkgdir/etc/kvmd/nginx" "$_cfg_default/nginx"/*.conf*
	chmod 644 "$pkgdir/etc/kvmd/nginx/"{nginx,ssl}.conf*

	mkdir -p "$pkgdir/etc/kvmd/janus"
	chmod 755 "$pkgdir/etc/kvmd/janus"
	install -Dm444 -t "$pkgdir/etc/kvmd/janus" "$_cfg_default/janus"/*.jcfg

	install -Dm644 -t "$pkgdir/etc/kvmd" "$_cfg_default/kvmd"/*.yaml
	install -Dm600 -t "$pkgdir/etc/kvmd" "$_cfg_default/kvmd"/*passwd
	install -Dm600 -t "$pkgdir/etc/kvmd" "$_cfg_default/kvmd"/*.secret
	install -Dm644 -t "$pkgdir/etc/kvmd" "$_cfg_default/kvmd"/web.css
	mkdir -p "$pkgdir/etc/kvmd/override.d"

	mkdir -p "$pkgdir/var/lib/kvmd/"{msd,pst}
}


for _variant in "${_variants[@]}"; do
	_platform=${_variant%:*}
	_board=${_variant#*:}
	_base=${_platform%-*}
	_video=${_platform#*-}
	eval "package_kvmd-platform-$_platform-$_board() {
		cd \"kvmd-\$pkgver\"

		pkgdesc=\"PiKVM platform configs - $_platform for $_board\"
		depends=(kvmd=$pkgver-$pkgrel \"linux-rpi-pikvm>=6.6.21-3\")

		backup=(
			etc/sysctl.d/99-kvmd.conf
			etc/udev/rules.d/99-kvmd.rules
			etc/kvmd/main.yaml
		)

		if [[ $_base == v0 ]]; then
			depends=(\"\${depends[@]}\" platformio-core avrdude make patch)
		elif [[ $_base == v4plus ]]; then
			depends=(\"\${depends[@]}\" flashrom-pikvm)
		fi

		if [[ $_platform =~ ^.*-hdmiusb$ ]]; then
			install -Dm755 -t \"\$pkgdir/usr/bin\" scripts/kvmd-udev-hdmiusb-check
		fi
		if [[ $_base == v4plus ]]; then
			install -Dm755 -t \"\$pkgdir/usr/bin\" scripts/kvmd-udev-restart-pass
		fi

		install -DTm644 configs/os/sysctl.conf \"\$pkgdir/etc/sysctl.d/99-kvmd.conf\"
		install -DTm644 configs/os/udev/common.rules \"\$pkgdir/usr/lib/udev/rules.d/99-kvmd-common.rules\"
		install -DTm644 configs/os/udev/$_platform-$_board.rules \"\$pkgdir/etc/udev/rules.d/99-kvmd.rules\"
		install -DTm444 configs/kvmd/main/$_platform-$_board.yaml \"\$pkgdir/etc/kvmd/main.yaml\"

		if [ -f configs/kvmd/fan/$_platform.ini ]; then
			backup=(\"\${backup[@]}\" etc/kvmd/fan.ini)
			depends=(\"\${depends[@]}\" \"kvmd-fan>=0.18\")
			install -DTm444 configs/kvmd/fan/$_platform.ini \"\$pkgdir/etc/kvmd/fan.ini\"
		fi

		if [ -f configs/os/modules-load/$_platform.conf ]; then
			backup=(\"\${backup[@]}\" etc/modules-load.d/kvmd.conf)
			install -DTm644 configs/os/modules-load/$_platform.conf \"\$pkgdir/etc/modules-load.d/kvmd.conf\"
		fi

		if [ -f configs/os/sudoers/$_platform ]; then
			backup=(\"\${backup[@]}\" etc/sudoers.d/99_kvmd)
			install -DTm440 configs/os/sudoers/$_platform \"\$pkgdir/etc/sudoers.d/99_kvmd\"
			chmod 750 \"\$pkgdir/etc/sudoers.d\"
		fi

		if [[ $_platform =~ ^.*-hdmi$ ]]; then
			backup=(\"\${backup[@]}\" etc/kvmd/tc358743-edid.hex)
			install -DTm444 configs/kvmd/edid/$_base.hex \"\$pkgdir/etc/kvmd/tc358743-edid.hex\"
		fi

		mkdir -p \"\$pkgdir/usr/share/kvmd\"
		local _device=\"\$pkgdir/usr/share/kvmd/device\"
		rm -f \"\$_device\"
		echo PIKVM_BASE=$_base > \"\$_device\"
		echo PIKVM_VIDEO=$_video >> \"\$_device\"
		echo PIKVM_BOARD=$_board >> \"\$_device\"
		chmod 444 \"\$_device\"
	}"
done
