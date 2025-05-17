FROM archlinux/archlinux:base

RUN mkdir -p /etc/pacman.d/hooks \
	&& ln -s /dev/null /etc/pacman.d/hooks/30-systemd-tmpfiles.hook

RUN echo 'Server = https://mirror.rackspace.com/archlinux/$repo/os/$arch' > /etc/pacman.d/mirrorlist \
	&& pacman-key --init \
	&& pacman-key --populate archlinux

RUN \
	--mount=type=cache,id=kvmd-pacman-pkg,target=/var/cache/pacman/pkg \
	--mount=type=cache,id=kvmd-pacman-db,target=/var/lib/pacman/sync \
	PACMAN="pacman --noconfirm --ask=4 --needed" \
	&& $PACMAN -Syy \
		archlinux-keyring \
	&& $PACMAN -S \
		glibc \
		pacman \
		openssl \
		openssl-1.1 \
	&& pacman-db-upgrade \
	&& $PACMAN -Syu \
		p11-kit \
		ca-certificates \
		ca-certificates-mozilla \
		ca-certificates-utils \
	&& $PACMAN -Syu \
		base-devel \
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
		python-build \
		python-wheel \
		python-setuptools \
		python-tox \
		python-mako \
		python-yaml \
		python-aiohttp \
		python-aiofiles \
		python-async-lru \
		python-passlib \
		python-bcrypt \
		python-pyotp \
		python-qrcode \
		python-pyserial \
		python-pyudev \
		python-evdev \
		python-setproctitle \
		python-psutil \
		python-netifaces \
		python-systemd \
		python-dbus \
		python-dbus-next \
		python-pygments \
		python-pam \
		python-pillow \
		python-xlib \
		python-mako \
		libxkbcommon \
		python-hidapi \
		python-ldap \
		python-zstandard \
		libgpiod \
		freetype2 \
		nginx-mainline \
		tesseract \
		tesseract-data-eng \
		tesseract-data-rus \
		ipmitool \
		socat \
		eslint \
		npm \
		shellcheck \
	&& :

COPY testenv/requirements.txt requirements.txt
RUN \
	--mount=type=cache,id=kvmd-pip,target=/root/.cache/pip \
	pip install --break-system-packages --root-user-action=ignore \
		-r requirements.txt

# https://stackoverflow.com/questions/57534295
WORKDIR /root
RUN \
	--mount=type=cache,id=kvmd-npm,target=/root/.npm \
	--mount=type=tmpfs,target=/tmp \
	npm install -g \
		htmlhint \
		pug \
		pug-cli \
		@babel/eslint-parser
WORKDIR /

ARG USTREAMER_MIN_VERSION
ENV USTREAMER_MIN_VERSION $USTREAMER_MIN_VERSION
RUN echo $USTREAMER_MIN_VERSION
RUN \
	--mount=type=tmpfs,target=/tmp \
	cd /tmp \
	&& git clone --depth=1 https://github.com/pikvm/ustreamer \
	&& cd ustreamer \
	&& make WITH_PYTHON=1 PREFIX=/usr DESTDIR=/ install

RUN mkdir -p \
		/etc/kvmd/{nginx,vnc} \
		/var/lib/kvmd/msd \
		/var/lib/kvmd/pst/data \
		/opt/vc/bin

COPY testenv/fakes/vcgencmd /usr/bin/
COPY testenv/fakes/sys /fake_sysfs/sys
COPY testenv/fakes/proc /fake_procfs/proc
COPY testenv/fakes/etc /fake_etc/etc

CMD ["/bin/bash"]
