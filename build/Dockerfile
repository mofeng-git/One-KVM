# syntax = docker/dockerfile:experimental
FROM python:3.12.0rc2-slim-bookworm AS builder

ARG TARGETARCH

RUN sed -i 's/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends build-essential libssl-dev libffi-dev python3-dev libevent-dev libjpeg-dev \
        libbsd-dev libudev-dev git pkg-config wget curl libmicrohttpd-dev libjansson-dev libssl-dev libsofia-sip-ua-dev libglib2.0-dev \
        libopus-dev libogg-dev libcurl4-openssl-dev liblua5.3-dev libconfig-dev libopus-dev libtool automake autoconf meson cmake \
        libx264-dev libyuv-dev libasound2-dev  libspeex-dev libspeexdsp-dev libopus-dev \
    && rm -rf /var/lib/apt/lists/*

COPY build/cargo_config /tmp/config

RUN --security=insecure pip config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple \
    && if [ ${TARGETARCH} = arm ]; then \
        mkdir -p /root/.cargo \
        && chmod 777 /root/.cargo && mount -t tmpfs none /root/.cargo \
        && export RUSTUP_DIST_SERVER="https://mirrors.tuna.tsinghua.edu.cn/rustup" \
        && export RUSTUP_UPDATE_ROOT="https://mirrors.ustc.edu.cn/rust-static/rustup" \
        && wget  https://sh.rustup.rs -O /root/rustup-init.sh \
        && sh /root/rustup-init.sh -y \
        && export PATH=$PATH:/root/.cargo/bin \
        && cp /tmp/config /root/.cargo/config.toml; \
    fi \
    && pip wheel --wheel-dir=/tmp/wheel/ cryptography

RUN pip install --no-cache-dir --root-user-action=ignore --disable-pip-version-check build \
    && pip wheel --wheel-dir=/tmp/wheel/ aiofiles aiohttp appdirs asn1crypto async_lru async-timeout bottle cffi chardet click colorama \
        dbus_next gpiod hidapi idna mako marshmallow more-itertools multidict netifaces packaging passlib pillow ply psutil pycparser \
        pyelftools pyghmi pygments pyparsing pyotp qrcode requests semantic-version setproctitle setuptools six spidev  \
        tabulate urllib3 wrapt xlib yarl pyserial pyyaml zstandard supervisor

RUN git clone --depth=1 https://gitlab.freedesktop.org/libnice/libnice /tmp/libnice \
    && cd /tmp/libnice \
    && meson --prefix=/usr build && ninja -C build && ninja -C build install

RUN curl https://github.com/cisco/libsrtp/archive/v2.2.0.tar.gz -L -o /tmp/libsrtp-2.2.0.tar.gz \
    && cd /tmp \
    && tar xfv libsrtp-2.2.0.tar.gz \
    && cd libsrtp-2.2.0 \
    && ./configure --prefix=/usr --enable-openssl \
    && make shared_library && make install

RUN git clone --depth=1 https://libwebsockets.org/repo/libwebsockets /tmp/libwebsockets \
    && cd /tmp/libwebsockets \
    && mkdir build && cd build \
    && cmake -DLWS_MAX_SMP=1 -DLWS_WITHOUT_EXTENSIONS=0 -DCMAKE_INSTALL_PREFIX:PATH=/usr -DCMAKE_C_FLAGS="-fpic" .. \
    && make && make install

RUN git clone --depth=1 https://github.com/meetecho/janus-gateway.git /tmp/janus-gateway \
    && cd /tmp/janus-gateway \
    && sh autogen.sh \
    && ./configure --enable-static --enable-websockets  --enable-plugin-audiobridge \
        --disable-data-channels --disable-rabbitmq --disable-mqtt --disable-all-plugins --disable-all-loggers \
        --prefix=/usr \
    && make && make install

RUN sed --in-place --expression 's|^#include "refcount.h"$|#include "../refcount.h"|g' /usr/include/janus/plugins/plugin.h \
    && git clone --depth=1 https://github.com/mofeng-git/ustreamer /tmp/ustreamer \
    && make -j WITH_PYTHON=1 WITH_JANUS=1 WITH_LIBX264=1 -C /tmp/ustreamer \
    && /tmp/ustreamer/ustreamer -v

RUN mkdir /tmp/lib \
    && cd /lib/*-linux-*/ \
    && cp libevent_core-*.so.7 libbsd.so.0 libevent_pthreads-*.so.7 libspeexdsp.so.1 libevent-*.so.7 libjpeg.so.62 libx264.so.164 libyuv.so.0 \
        libnice.so.10 /usr/lib/libsrtp2.so.1 /usr/lib/libwebsockets.so.19 \
        /tmp/lib/ \
    && cp /tmp/ustreamer/python/dist/*.whl /tmp/wheel/

FROM python:3.12.0rc2-slim-bookworm

LABEL maintainer="mofeng654321@hotmail.com"

COPY --from=builder /tmp/lib/* /tmp/lib/
COPY --from=builder /tmp/ustreamer/ustreamer /tmp/ustreamer/ustreamer-dump /usr/bin/janus /usr/bin/
COPY --from=builder /tmp/wheel/*.whl /tmp/wheel/
COPY --from=builder /tmp/ustreamer/libjanus_ustreamer.so /usr/lib/ustreamer/janus/
COPY --from=builder /usr/lib/janus/transports/* /usr/lib/janus/transports/

ARG TARGETARCH

ENV PYTHONDONTWRITEBYTECODE 1
ENV PYTHONUNBUFFERED 1
ENV TZ=Asia/Shanghai

RUN cp /tmp/lib/* /lib/*-linux-*/ \
    && pip install --no-cache-dir --root-user-action=ignore --disable-pip-version-check /tmp/wheel/*.whl \
    && rm -rf /tmp/lib /tmp/wheel

RUN sed -i 's/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends libxkbcommon-x11-0 nginx tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim iptables sudo  curl \
        libmicrohttpd12 libjansson4 libssl3 libsofia-sip-ua0 libglib2.0-0 libopus0 libogg0 libcurl4 libconfig9 libusrsctp2 libwebsockets17 libnss3 libasound2 \
    && rm -rf /var/lib/apt/lists/*

RUN if [ ${TARGETARCH} = arm ]; then ARCH=armhf; elif [ ${TARGETARCH} = arm64 ]; then ARCH=aarch64; elif [ ${TARGETARCH} = amd64 ]; then ARCH=x86_64; fi \
    && curl https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.$ARCH -L -o /usr/local/bin/ttyd \
    && chmod +x /usr/local/bin/ttyd \
    && adduser kvmd --gecos "" --disabled-password \
    && ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata \
    && mkdir -p /etc/kvmd_backup/override.d /var/lib/kvmd/msd /var/lib/kvmd/pst/data /opt/vc/bin /run/kvmd /tmp/kvmd-nginx \
    && touch /run/kvmd/ustreamer.sock


COPY testenv/fakes/vcgencmd /usr/bin/
COPY extras/ /usr/share/kvmd/extras/
COPY web/ /usr/share/kvmd/web/
COPY build/platform scripts/kvmd-gencert /usr/share/kvmd/
COPY contrib/keymaps /usr/share/kvmd/keymaps
COPY kvmd/ build/init.sh /kvmd/
COPY configs/kvmd/* /etc/kvmd_backup/
COPY configs/nginx/* /etc/kvmd_backup/nginx/
COPY configs/janus/* /etc/kvmd_backup/janus/
COPY configs/hw_info/* /etc/kvmd_backup/hw_info/
COPY testenv/js/* /usr/share/janus/javascript/

ENTRYPOINT ["/kvmd/init.sh"]