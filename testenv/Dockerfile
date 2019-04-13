FROM archlinux/base

RUN dd if=/dev/zero of=/root/loop.img bs=1024 count=1048576

RUN pacman -Syu --noconfirm \
	&& pacman -S --needed --noconfirm \
		base \
		base-devel \
		git \
		expac \
		jshon \
	&& (pacman -Sc --noconfirm || true)

RUN useradd -r -c "Packer build user" -s /sbin/nologin packer \
	&& cd /tmp \
	&& sudo -u packer git clone https://aur.archlinux.org/packer-kit.git \
	&& cd packer-kit \
	&& sudo -u packer makepkg \
	&& pacman --noconfirm -U packer-kit-*.pkg.tar.xz \
	&& cd - \
	&& rm -rf /tmp/packer-kit

RUN pacman -Syy \
	&& mkdir /.npm /home/packer \
	&& chmod 777 /.npm /home/packer \
	&& packer-user -S --needed --noconfirm \
		python \
		python-pip \
		python-tox \
		python-systemd \
		python-dbus \
		python-mako \
		nginx-mainline \
		ustreamer \
		socat \
		htmlhint \
		eslint \
	&& rm -rf /.npm /home/packer \
	&& (pacman -Sc --noconfirm || true)

RUN mkdir -p /etc/kvmd/nginx

CMD /bin/bash
