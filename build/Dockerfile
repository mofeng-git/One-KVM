# syntax = docker/dockerfile:experimental
FROM python:3.12.0rc2-slim-bookworm AS builder

ARG TARGETARCH

RUN sed -i 's/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends build-essential libssl-dev libffi-dev python3-dev libevent-dev libjpeg-dev libbsd-dev libudev-dev git pkg-config wget curl

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

RUN apt-get install -y libx264-dev libyuv-dev janus-dev libasound2-dev  libspeex-dev libspeexdsp-dev libopus-dev \
    && sed --in-place --expression 's|^#include "refcount.h"$|#include "../refcount.h"|g' /usr/include/janus/plugins/plugin.h \
    && git clone --depth=1 https://github.com/mofeng-git/ustreamer /tmp/ustreamer \
    && make -j WITH_PYTHON=1 WITH_JANUS=1 WITH_LIBX264=1 -C /tmp/ustreamer \
    && /tmp/ustreamer/ustreamer -v

RUN mkdir /tmp/lib \
    && cp  /lib/*-linux-*/libevent_core-*.so.7 /lib/*-linux-*/libbsd.so.0 /lib/*-linux-*/libevent_pthreads-*.so.7 /lib/*-linux-*/libspeexdsp.so.1 \
        /lib/*-linux-*/libevent-*.so.7 /lib/*-linux-*/libjpeg.so.62 /lib/*-linux-*/libx264.so.164 /lib/*-linux-*/libyuv.so.0 /lib/*-linux-*/libasound.so.2 /tmp/lib/ \
    && cp /tmp/ustreamer/python/dist/*.whl /tmp/wheel/

FROM python:3.12.0rc2-slim-bookworm

LABEL maintainer="mofeng654321@hotmail.com"

COPY --from=builder /tmp/lib/* /tmp/lib/
COPY --from=builder /tmp/ustreamer/ustreamer /tmp/ustreamer/ustreamer-dump /usr/local/bin/
COPY --from=builder /tmp/wheel/*.whl /tmp/wheel/
COPY --from=builder /tmp/ustreamer/libjanus_ustreamer.so /usr/lib/ustreamer/janus/


ENV PYTHONDONTWRITEBYTECODE 1
ENV PYTHONUNBUFFERED 1


RUN cp /tmp/lib/* /lib/*-linux-*/ \
    && pip install --no-cache-dir --root-user-action=ignore --disable-pip-version-check /tmp/wheel/*.whl \
    && rm -rf /tmp/lib /tmp/wheel

RUN sed -i 's/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends libxkbcommon-x11-0 nginx tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim iptables sudo janus \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p \
    /etc/kvmd/nginx \
    /etc/kvmd/vnc \
    /etc/kvmd/override.d\
    /etc/kvmd/hw_info/ \
    /var/lib/kvmd/msd \
    /var/lib/kvmd/pst/data \
    /opt/vc/bin \
    /run/kvmd \
    /tmp/kvmd-nginx

COPY testenv/fakes/vcgencmd /usr/bin/
COPY extras/ /usr/share/kvmd/extras/
COPY web/ /usr/share/kvmd/web/
COPY build/platform scripts/kvmd-gencert /usr/share/kvmd/
COPY contrib/keymaps /usr/share/kvmd/keymaps
COPY kvmd/ /kvmd
COPY configs/kvmd/* /etc/kvmd/
COPY configs/nginx/* /etc/kvmd/nginx/
COPY configs/janus/* /etc/kvmd/janus/
COPY configs/hw_info/* /etc/kvmd/hw_info/
COPY build/v2-hdmiusb-rpi4.override.yaml /etc/kvmd/override.yaml
COPY configs/kvmd/main/v2-hdmiusb-rpi4.yaml /etc/kvmd/main.yaml
COPY testenv/js/* /usr/share/janus/javascript/

RUN touch /run/kvmd/ustreamer.sock /etc/kvmd/.docker_flag \
    && adduser kvmd --gecos "" --disabled-password \
    && ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata

#RUN bash /etc/kvmd/init.sh

EXPOSE 4430
CMD ["/etc/kvmd/init.sh"]