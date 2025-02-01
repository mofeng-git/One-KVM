#!/bin/bash

# 定义颜色代码
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 输出日志的函数
log_info() {
    echo -e "${GREEN}[INFO] $1${NC}"
}

log_warn() {
    echo -e "${YELLOW}[WARN] $1${NC}"
}

log_error() {
    echo -e "${RED}[ERROR] $1${NC}"
}

# 初始化检查
log_info "One-KVM 正在启动..."

# 首次初始化配置
if [ ! -f /etc/kvmd/.init_flag ]; then
    log_info "首次初始化配置..."
    
    # 创建必要目录并移动配置文件
    if mkdir -p /etc/kvmd/ && \
       mv /etc/kvmd_backup/* /etc/kvmd/ && \
       touch /etc/kvmd/.docker_flag && \
       sed -i 's/localhost.localdomain/docker/g' /etc/kvmd/meta.yaml && \
       sed -i 's/localhost/localhost:4430/g' /etc/kvmd/kvm_input.sh; then
        log_info "基础配置完成"
    else
        log_error "基础配置失败"
        exit 1
    fi

    # SSL证书配置
    if ! /usr/share/kvmd/kvmd-gencert --do-the-thing && \
         ! /usr/share/kvmd/kvmd-gencert --do-the-thing --vnc; then
        log_error "SSL 证书生成失败"
        exit 1
    fi

    # SSL开关配置
    if [ "$NOSSL" == 1 ]; then
        log_info "已禁用SSL"
        if ! python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf -o nginx/https/enabled=false; then
            log_error "Nginx 配置失败"
            exit 1
        fi
    else
        if ! python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf; then
            log_error "Nginx 配置失败"
            exit 1
        fi
    fi

    # 认证配置
    if [ "$NOAUTH" == "1" ]; then
        sed -i "s/enabled: true/enabled: false/g" /etc/kvmd/override.yaml
        log_info "已禁用认证"
    fi

    #add supervisord conf
    if [ "$NOWEBTERM" == "1" ]; then
        log_info "已禁用 WebTerm 功能"
        rm -r /usr/share/kvmd/extras/webterm
    else
        cat >> /etc/kvmd/supervisord.conf  << EOF

[program:kvmd-webterm]
command=/usr/local/bin/ttyd --interface=/run/kvmd/ttyd.sock --port=0 --writable /bin/bash -c '/etc/kvmd/armbain-motd; bash'
directory=/
autostart=true
autorestart=true
priority=14
stopasgroup=true
stdout_logfile=/dev/stdout
stdout_logfile_maxbytes = 0
redirect_stderr=true
EOF
    fi

    if [ "$NOWEBTERMWRITE" == "1" ]; then
        sed -i "s/--writable//g" /etc/kvmd/supervisord.conf
    fi

    if [  "$NOVNC" == "1" ]; then
        log_info "已禁用 VNC 功能"
        rm -r /usr/share/kvmd/extras/vnc
    else
        cat >> /etc/kvmd/supervisord.conf << EOF

[program:kvmd-vnc]
command=python -m kvmd.apps.vnc --run
directory=/
autostart=true
autorestart=true
priority=11
stopasgroup=true
stdout_logfile=/dev/stdout
stdout_logfile_maxbytes = 0
redirect_stderr=true
EOF
    fi

    if [  "$NOIPMI" == "1" ]; then
        log_info "已禁用IPMI功能"
        rm -r /usr/share/kvmd/extras/ipmi
    else
        cat >> /etc/kvmd/supervisord.conf << EOF

[program:kvmd-ipmi]
command=python -m kvmd.apps.ipmi --run
directory=/
autostart=true
autorestart=true
priority=12
stopasgroup=true
stdout_logfile=/dev/stdout
stdout_logfile_maxbytes = 0
redirect_stderr=true
EOF
    fi

    #switch OTG config
    if [ "$OTG" == "1" ]; then
        log_info "已启用 OTG 功能"
        sed -i "s/ch9329/otg/g" /etc/kvmd/override.yaml
        sed -i "s/device: \/dev\/ttyUSB0//g" /etc/kvmd/override.yaml
        if [ "$NOMSD" == 1 ]; then
            log_info "已禁用 MSD 功能"
        else
            sed -i "s/#type: otg/type: otg/g" /etc/kvmd/override.yaml
        fi
    fi

    if [ ! -z "$VIDEONUM" ]; then
        if sed -i "s/\/dev\/video0/\/dev\/video$VIDEONUM/g" /etc/kvmd/override.yaml && \
           sed -i "s/\/dev\/video0/\/dev\/video$VIDEONUM/g" /etc/kvmd/janus/janus.plugin.ustreamer.jcfg; then
            log_info "视频设备已设置为 /dev/video$VIDEONUM"
        fi
    fi

    if [ ! -z "$AUDIONUM" ]; then
        if sed -i "s/hw:0/hw:$AUDIONUM/g" /etc/kvmd/janus/janus.plugin.ustreamer.jcfg; then
            log_info "音频设备已设置为 hw:$AUDIONUM"
        fi
    fi

    if [ ! -z "$CH9329SPEED" ]; then
        if sed -i "s/speed: 9600/speed: $CH9329SPEED/g" /etc/kvmd/override.yaml; then
            log_info "CH9329 串口速率已设置为 $CH9329SPEED"
        fi
    fi

    if [ ! -z "$CH9329TIMEOUT" ]; then
        if sed -i "s/read_timeout: 0.3/read_timeout: $CH9329TIMEOUT/g" /etc/kvmd/override.yaml; then
            log_info "CH9329 超时已设置为 $CH9329TIMEOUT 秒"
        fi
    fi

    if [ ! -z "$VIDEOFORMAT" ]; then
        if sed -i "s/format=mjpeg/format=$VIDFORMAT/g" /etc/kvmd/override.yaml; then
            log_info "视频输入格式已设置为 $VIDFORMAT"
        fi
    fi
    
    touch /etc/kvmd/.init_flag
    log_info "初始化配置完成"
fi

# OTG设备配置
if [ "$OTG" == "1" ]; then
    log_info "正在配置 OTG 设备..."
    rm -r /run/kvmd/otg &> /dev/null
    
    if ! modprobe libcomposite; then
        log_error "加载 libcomposite 模块失败"
        exit 1
    fi

    if python -m kvmd.apps.otg start; then
        ln -s /dev/hidg1 /dev/kvmd-hid-mouse
        ln -s /dev/hidg0 /dev/kvmd-hid-keyboard
        ln -s /dev/hidg2 /dev/kvmd-hid-mouse-alt
        log_info "OTG 设备配置完成"
    else
        log_error "OTG 设备挂载失败"
        exit 1
    fi
fi

log_info "One-KVM 启动完成，正在启动服务..."
exec supervisord -c /etc/kvmd/supervisord.conf