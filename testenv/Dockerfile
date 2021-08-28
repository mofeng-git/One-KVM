FROM archlinux/archlinux:base-devel

RUN mkdir -p /etc/pacman.d/hooks \
	&& ln -s /dev/null /etc/pacman.d/hooks/30-systemd-tmpfiles.hook

RUN echo "Server = http://mirror.yandex.ru/archlinux/\$repo/os/\$arch" > /etc/pacman.d/mirrorlist

RUN pacman -Syu --noconfirm \
	&& pacman -S --needed --noconfirm \
		autoconf-archive \
		help2man \
		m4 \
		vim \
		git \
		libjpeg \
		libevent \
		libutil-linux \
		libbsd \
		python \
		python-pip \
		python-tox \
		python-mako \
		python-yaml \
		python-aiohttp \
		python-aiofiles \
		python-periphery \
		python-passlib \
		python-pyserial \
		python-setproctitle \
		python-psutil \
		python-netifaces \
		python-systemd \
		python-dbus \
		python-pygments \
		python-pam \
		python-pillow \
		python-xlib \
		python-hidapi \
		freetype2 \
		nginx-mainline \
		ipmitool \
		socat \
		eslint \
		npm \
	&& (pacman -Sc --noconfirm || true) \
	&& rm -rf /var/cache/pacman/pkg/*

COPY testenv/requirements.txt requirements.txt
RUN pip install -r requirements.txt

# https://stackoverflow.com/questions/57534295
WORKDIR /root
RUN npm install htmlhint -g \
	&& npm install pug \
	&& npm install pug-cli -g \
	&& npm install @babel/eslint-parser -g
WORKDIR /

ARG LIBGPIOD_VERSION
ENV LIBGPIOD_PKG libgpiod-$LIBGPIOD_VERSION
RUN curl \
		-o $LIBGPIOD_PKG.tar.gz \
		https://git.kernel.org/pub/scm/libs/libgpiod/libgpiod.git/snapshot/$LIBGPIOD_PKG.tar.gz \
	&& tar -xzvf $LIBGPIOD_PKG.tar.gz \
	&& cd $LIBGPIOD_PKG \
	&& ./autogen.sh --prefix=/usr --enable-tools=yes --enable-bindings-python \
	&& make PREFIX=/usr install \
	&& cd - \
	&& rm -rf $LIBGPIOD_PKG{,.tar.gz}

ARG USTREAMER_MIN_VERSION
ENV USTREAMER_MIN_VERSION $USTREAMER_MIN_VERSION
RUN echo $USTREAMER_MIN_VERSION
RUN git clone https://github.com/pikvm/ustreamer \
	&& cd ustreamer \
	&& make WITH_PYTHON=1 PREFIX=/usr DESTDIR=/ install \
	&& cd - \
	&& rm -rf ustreamer

RUN mkdir -p \
		/etc/kvmd/{nginx,vnc} \
		/var/lib/kvmd/msd/{images,meta} \
		/opt/vc/bin \
		/fake_sysfs/sys/kernel/config/usb_gadget/kvmd/functions/mass_storage.usb0/lun.0 \
		/fake_sysfs/sys/class/thermal/thermal_zone0 \
		/fake_procfs/proc/device-tree \
		/fake_sysfs/sys/class/udc/fe980000.usb/device \
		/fake_sysfs/sys/bus/platform/drivers/dwc2 \
	&& echo configured > /fake_sysfs/sys/class/udc/fe980000.usb/state \
	&& ln -s /fake_sysfs/sys/bus/platform/drivers/dwc2 /fake_sysfs/sys/class/udc/fe980000.usb/device/driver

COPY testenv/fakes/vcgencmd /opt/vc/bin/
COPY testenv/fakes/msd/* /fake_sysfs/sys/kernel/config/usb_gadget/kvmd/functions/mass_storage.usb0/lun.0/
COPY testenv/fakes/cpu_temp /fake_sysfs/sys/class/thermal/thermal_zone0/temp
COPY testenv/fakes/dt_model /fake_procfs/proc/device-tree/model

CMD /bin/bash
