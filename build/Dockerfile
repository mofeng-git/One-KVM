FROM registry.cn-hangzhou.aliyuncs.com/silentwind/kvmd-stage-0 AS builder

FROM python:3.11.11-slim-bookworm

LABEL maintainer="mofeng654321@hotmail.com"

COPY --from=builder /tmp/lib/* /tmp/lib/
COPY --from=builder /tmp/ustreamer/ustreamer /tmp/ustreamer/ustreamer-dump /usr/bin/janus /usr/bin/
COPY --from=builder /tmp/wheel/*.whl /tmp/wheel/
COPY --from=builder /tmp/ustreamer/libjanus_ustreamer.so /usr/lib/ustreamer/janus/
COPY --from=builder /usr/lib/janus/transports/* /usr/lib/janus/transports/

ARG TARGETARCH

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    TZ=Asia/Shanghai

RUN sed -i 's/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        libxkbcommon-x11-0 \
        nginx \
        tesseract-ocr \
        tesseract-ocr-eng \
        tesseract-ocr-chi-sim \
        iptables \
        sudo \
        curl \
        kmod \
        libmicrohttpd12 \
        libjansson4 \
        libssl3 \
        libsofia-sip-ua0 \
        libglib2.0-0 \
        libopus0 \
        libogg0 \
        libcurl4 \
        libconfig9 \
        libusrsctp2 \
        libwebsockets17 \
        libnss3 \
        libasound2 \
        nano \
        unzip \
        libavcodec59 \
        libavformat59 \
        libavutil57 \
        libswscale6 \
        libavfilter8 \
        libavdevice59 \
    && if [ ${TARGETARCH} != arm ] && [ ${TARGETARCH} != arm64 ]; then \
        apt-get install -y --no-install-recommends \
            ffmpeg \
            vainfo \
            libva2 \
            libva-drm2 \
            libva-x11-2 \
            libdrm2 \
            mesa-va-drivers \
            mesa-vdpau-drivers \
            intel-media-va-driver \
            i965-va-driver; \
    fi \
    && if [ ${TARGETARCH} = arm ] || [ ${TARGETARCH} = arm64 ]; then \
        apt-get install -y --no-install-recommends \
            v4l-utils \
            libv4l-0; \
    fi \
    && cp /tmp/lib/* /lib/*-linux-*/ \
    && pip install --no-cache-dir --root-user-action=ignore --disable-pip-version-check /tmp/wheel/*.whl \
    && pip install --no-cache-dir --root-user-action=ignore --disable-pip-version-check pyfatfs \
    && if [ ${TARGETARCH} = arm ]; then ARCH=armhf; \
       elif [ ${TARGETARCH} = arm64 ]; then ARCH=aarch64; \
       elif [ ${TARGETARCH} = amd64 ]; then ARCH=x86_64; \
       fi \
    && curl https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.$ARCH -L -o /usr/local/bin/ttyd \
    && chmod +x /usr/local/bin/ttyd \
    && adduser kvmd --gecos "" --disabled-password \
    && ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata \
    && mkdir -p /etc/kvmd_backup/override.d \
               /var/lib/kvmd/msd/images \
               /var/lib/kvmd/msd/meta \
               /var/lib/kvmd/pst/data \
               /var/lib/kvmd/msd/NormalFiles \
               /opt/vc/bin \
               /run/kvmd \
               /tmp/kvmd-nginx \
    && touch /run/kvmd/ustreamer.sock \
	&& apt clean \
    && rm -rf /var/lib/apt/lists/* \
    && rm -rf /tmp/lib /tmp/wheel

COPY testenv/fakes/vcgencmd scripts/kvmd* /usr/bin/
COPY extras/ /usr/share/kvmd/extras/
COPY web/ /usr/share/kvmd/web/
COPY scripts/kvmd-gencert /usr/share/kvmd/
COPY build/platform/docker /usr/share/kvmd/platform
COPY contrib/keymaps /usr/share/kvmd/keymaps
COPY kvmd/ build/init.sh /kvmd/
COPY configs/kvmd/ /etc/kvmd_backup/
COPY configs/nginx/ /etc/kvmd_backup/nginx/
COPY configs/janus/ /etc/kvmd_backup/janus/
COPY configs/hw_info/ /etc/kvmd_backup/hw_info/
COPY testenv/js/ /usr/share/janus/javascript/

ENTRYPOINT ["/kvmd/init.sh"]