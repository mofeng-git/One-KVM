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
	&& sudo -u packer git clone https://github.com/pi-kvm/packer-kit.git \
	&& cd packer-kit \
	&& sudo -u packer makepkg \
	&& pacman --noconfirm -U packer-kit-*.pkg.tar.xz \
	&& cd - \
	&& rm -rf /tmp/packer-kit

RUN pkg-install -S \
	python \
	python-pip \
	python-tox \
	python-systemd \
	python-dbus \
	python-mako \
	nginx-mainline \
	socat \
	htmlhint \
	eslint

ARG USTREAMER_VERSION
ENV USTREAMER_VERSION $USTREAMER_VERSION
RUN echo $USTREAMER_VERSION
RUN pkg-install -S ustreamer

COPY testenv/requirements.txt requirements.txt
RUN pip install -r requirements.txt

RUN mkdir -p /etc/kvmd/nginx

CMD /bin/bash
