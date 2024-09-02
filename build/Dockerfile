FROM python:3.12.0rc2-slim-bookworm as builder

RUN sed -i 's/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends build-essential libevent-dev libjpeg-dev libbsd-dev git pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN pip config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple \
    && pip install --no-cache-dir --root-user-action=ignore --disable-pip-version-check build \
    && pip wheel --wheel-dir=/tmp/wheel/ aiofiles aiohttp appdirs asn1crypto async_lru async-timeout bottle cffi chardet click colorama cryptography \
        dbus_next gpiod hidapi idna mako marshmallow more-itertools multidict netifaces packaging passlib pillow ply psutil pycparser \
        pyelftools pyghmi pygments pyparsing pyotp qrcode requests semantic-version setproctitle setuptools six spidev  \
        tabulate urllib3 wrapt xlib yarl pyserial pyyaml zstandard supervisor \
    && pip cache purge

RUN git clone --depth=1 https://gh.xmly.dev/https://github.com/pikvm/ustreamer /tmp/ustreamer \
    #&& git clone --depth=1 https://github.com/pikvm/ustreamer /tmp/ustreamer \
    && make -j WITH_PYTHON=1 -C /tmp/ustreamer \
    && /tmp/ustreamer/ustreamer -v

RUN mkdir /tmp/lib \
    && cp  /lib/*-linux-gnu/libevent_core-*.so.7 /lib/*-linux-gnu/libbsd.so.0 /lib/*-linux-gnu/libevent_pthreads-*.so.7 \
        /lib/*-linux-gnu/libevent-*.so.7 /lib/*-linux-gnu/libjpeg.so.62 /tmp/lib/ \
    && cp /tmp/ustreamer/python/dist/*.whl /tmp/wheel/

FROM python:3.12.0rc2-slim-bookworm

LABEL maintainer="mofeng654321@hotmail.com"

COPY --from=builder /tmp/lib/* /tmp/lib/
COPY --from=builder /tmp/ustreamer/ustreamer /tmp/ustreamer/ustreamer-dump /usr/local/bin/
COPY --from=builder /tmp/wheel/*.whl /tmp/wheel/

ENV PYTHONDONTWRITEBYTECODE 1
ENV PYTHONUNBUFFERED 1


RUN cp /tmp/lib/* /lib/*-linux-gnu/ \
    && pip install --no-cache-dir --root-user-action=ignore --disable-pip-version-check /tmp/wheel/*.whl \
    && rm -rf /tmp/lib /tmp/wheel

RUN sed -i 's/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends libxkbcommon-x11-0 nginx tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim iptables sudo \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p \
    /etc/kvmd/nginx \
    /etc/kvmd/vnc \
    /etc/kvmd/override.d\
    /var/lib/kvmd/msd \
    /var/lib/kvmd/pst/data \
    /opt/vc/bin \
    /run/kvmd \
    /tmp/kvmd-nginx

COPY testenv/fakes/vcgencmd /usr/bin/
COPY extras/ /usr/share/kvmd/extras/
COPY web/ /usr/share/kvmd/web/
COPY testenv/platform/ scripts/kvmd-gencert /usr/share/kvmd/
COPY contrib/keymaps /usr/share/kvmd/keymaps
COPY kvmd/ /kvmd
COPY configs/kvmd/*.yaml configs/kvmd/*passwd build/supervisord.conf /etc/kvmd/
COPY configs/nginx/* /etc/kvmd/nginx/
COPY build/v2-hdmiusb-rpi4.override.yaml /etc/kvmd/override.yaml
COPY configs/kvmd/main/v2-hdmiusb-rpi4.yaml /etc/kvmd/main.yaml

RUN touch /run/kvmd/ustreamer.sock \
    && touch /etc/kvmd/totp.secret \
    && /usr/share/kvmd/kvmd-gencert --do-the-thing \
    && /usr/share/kvmd/kvmd-gencert --do-the-thing --vnc \
    && ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata \
    && python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf

EXPOSE 4430
CMD ["supervisord", "-c", "/etc/kvmd/supervisord.conf"]