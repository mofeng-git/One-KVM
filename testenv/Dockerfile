FROM archlinux/base

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
		python-hidapi \
		nginx-mainline \
		socat \
		eslint \
		npm \
	&& (pacman -Sc --noconfirm || true)

RUN npm install htmlhint -g \
	&& npm install pug \
	&& npm install pug-cli -g

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
		/opt/vc/bin \
		/fake_sysfs/sys/kernel/config/usb_gadget/kvmd/functions/mass_storage.usb0/lun.0 \
		/fake_sysfs/sys/class/thermal/thermal_zone0 \
		/fake_procfs/proc/device-tree

COPY testenv/fakes/vcgencmd /opt/vc/bin/
COPY testenv/fakes/msd/* /fake_sysfs/sys/kernel/config/usb_gadget/kvmd/functions/mass_storage.usb0/lun.0/
COPY testenv/fakes/cpu_temp /fake_sysfs/sys/class/thermal/thermal_zone0/temp
COPY testenv/fakes/dt_model /fake_procfs/proc/device-tree/model

CMD /bin/bash
