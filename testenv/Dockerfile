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

RUN useradd -r -c "Packer build user" -m -d /var/packer -s /sbin/nologin packer \
	&& cd /tmp \
	&& sudo -u packer git clone https://aur.archlinux.org/packer-kit.git \
	&& cd packer-kit \
	&& sudo -u packer makepkg \
	&& pacman --noconfirm -U packer-kit-*.pkg.tar.xz \
	&& cd - \
	&& rm -rf /tmp/packer-kit

RUN pkg-install \
	python \
	python-pip \
	python-tox \
	python-systemd \
	python-dbus \
	python-mako \
	libevent-patched \
	nginx-mainline \
	ustreamer \
	socat \
	htmlhint \
	eslint

COPY testenv/requirements.txt requirements.txt
RUN pip install -r requirements.txt

RUN useradd -r -c "Pi-KVM Nginx Server" -s /sbin/nologin kvmd-nginx
RUN mkdir -p /etc/kvmd/nginx

CMD /bin/bash
