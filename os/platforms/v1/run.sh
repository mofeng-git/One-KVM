#!/bin/sh
set -e
set -x

. ../functions.sh


pkg_install \
	kvmd \
	mjpg-streamer-pikvm \
	nginx

cp config.txt "$FS/boot/"
cp 99-pikvm.conf "$FS/etc/sysctl.d/"
cp index.html "$FS/srv/http/"
cp kvmd.yaml "$FS/etc/"
cp nginx.conf "$FS/etc/nginx/"

rpi systemctl enable kvmd
rpi systemctl enable nginx
