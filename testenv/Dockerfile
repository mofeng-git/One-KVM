FROM archlinux/base

RUN dd if=/dev/zero of=/root/loop.img bs=1024 count=1048576

RUN echo "Server = http://mirror.yandex.ru/archlinux/\$repo/os/\$arch" > /etc/pacman.d/mirrorlist

RUN pacman -Syu --noconfirm \
	&& pacman -S --needed --noconfirm \
		base \
		base-devel \
		vim \
		git \
		libjpeg \
		libevent \
		libutil-linux \
		libbsd \
		python \
		python-pip \
		python-tox \
		python-systemd \
		python-dbus \
		python-psutil \
		python-mako \
		nginx-mainline \
		socat \
		eslint \
		npm \
	&& (pacman -Sc --noconfirm || true)

RUN npm install htmlhint -g

ARG USTREAMER_MIN_VERSION
ENV USTREAMER_MIN_VERSION $USTREAMER_MIN_VERSION
RUN echo $USTREAMER_MIN_VERSION
RUN git clone https://github.com/pikvm/ustreamer \
	&& cd ustreamer \
	&& make PREFIX=/usr install \
	&& cd - \
	&& rm -rf ustreamer

COPY testenv/requirements.txt requirements.txt
RUN pip install -r requirements.txt

RUN mkdir -p \
		/etc/kvmd/nginx \
		/var/lib/kvmd/msd/{images,meta} \
		/fake_sysfs/sys/kernel/config/usb_gadget/kvmd/functions/mass_storage.usb0/lun.0 \
	&& cd /fake_sysfs/sys/kernel/config/usb_gadget/kvmd/functions/mass_storage.usb0/lun.0 \
	&& touch file  \
	&& echo 1 > cdrom \
	&& echo 1 > ro

CMD /bin/bash
