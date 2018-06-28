#!/bin/sh
set -e
set -x

. ../functions.sh


cat config.txt > "$FS/boot/config.txt"
pkg_install \
	kvmd \
	mjpg-streamer-pikvm \
	nginx

cp index.html "$FS/srv/http/"
cp nginx.conf "$FS/etc/nginx/"
rpi systemctl enable kvmd
rpi systemctl enable nginx
