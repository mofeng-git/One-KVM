FROM base/archlinux

RUN dd if=/dev/zero of=/root/loop.img bs=1024 count=1048576

RUN pacman -Syu --noconfirm \
	&& pacman -S --noconfirm \
		git \
		patch \
		make \
		fakeroot \
		binutils \
		expac \
		jshon \
		sudo \
		systemd \
		pkgconf \
	&& pacman -Sc --noconfirm

RUN useradd -r -d / packer \
	&& cd /tmp \
	&& sudo -u packer git clone https://aur.archlinux.org/packer-color.git \
	&& cd packer-color \
	&& sudo -u packer makepkg \
	&& pacman --noconfirm -U packer-color-*.pkg.tar.xz \
	&& ln -s /usr/bin/packer-color /usr/local/bin/packer \
	&& cp /usr/bin/packer-color /usr/local/bin/user-packer \
	&& sed -i -e "s|makepkg \$MAKEPKGOPTS |chown -R packer:packer \$dir; makepkg \$MAKEPKGOPTS |g" \
		/usr/local/bin/user-packer \
	&& sed -i -e "s|makepkg \$MAKEPKGOPTS --asroot -f|sudo -u packer makepkg \$MAKEPKGOPTS -f|g" \
		/usr/local/bin/user-packer \
	&& cd - \
	&& rm -rf /tmp/packer-color

RUN pacman -Syy \
	&& mkdir /.npm \
	&& chmod 777 /.npm \
	&& user-packer -S --noconfirm \
		python \
		python-pip \
		python-systemd \
		python-dbus \
		nginx-mainline \
		ustreamer \
		socat \
		htmlhint \
		eslint \
	&& rm -rf /.npm \
	&& pacman -Sc --noconfirm

COPY testenv/requirements.txt requirements.txt
RUN pip install -r requirements.txt

RUN mkdir -p /etc/kvmd/nginx

CMD /bin/bash
