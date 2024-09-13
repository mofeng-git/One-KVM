#!/bin/bash

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}One-KVM pre-starting...${NC}"

#仅首次运行，用于初始化配置文件
if [ ! -f /etc/kvmd/.init_flag ]; then
    #生成 ssl 证书 和 vnc 证书
    /usr/share/kvmd/kvmd-gencert --do-the-thing
    /usr/share/kvmd/kvmd-gencert --do-the-thing --vnc
    #生成 nginx 配置文件
    if [ "$NOSSL" = 1 ]; then
        python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf  -o nginx/https/enabled=false
    else
        python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf
    fi
    #OTG 初始化修改默认配置文件
    if [ "$OTG" == "1" ]; then
        echo -e "${GREEN}One-KVM OTG is enabled.${NC}"
        sed -i "s/ch9329/otg/g" /etc/kvmd/override.yaml
	    sed -i "s/device: \/dev\/ttyUSB0//g" /etc/kvmd/override.yaml
        cat >> /etc/kvmd/supervisord.conf << EOF

[program:kvmd-otg]
command=python -m kvmd.apps.otg start
directory=/
autostart=true
autorestart=unexpected
priority=9
stopasgroup=true
stdout_logfile=/dev/stdout
stdout_logfile_maxbytes = 0
redirect_stderr=true
EOF
    fi
    #/dev/video0 设备优先级高于 /dev/kvmd-video
    if [ -f /dev/video0 ]; then
        echo -e "${GREEN}Found /dev/video0, use it as kvmd video device.${NC}"
        sed -i "s/\/dev\/kvmd-video/\/dev\/video0/g" /etc/kvmd/override.yaml
    fi
    #设置用户账号密码
    if [ ! -z "$USERNAME" ] && [ ! -z "$PASSWORD" ]; then
        python -m kvmd.apps.htpasswd del admin
        echo $PASSWORD | python -m kvmd.apps.htpasswd set -i  "$USERNAME"
        echo "$PASSWORD -> $USERNAME:$PASSWORD" > /etc/kvmd/vncpasswd
        echo "$USERNAME:$PASSWORD -> $USERNAME:$PASSWORD" > /etc/kvmd/ipmipasswd
    else
        echo -e "${YELLOW} USERNAME and PASSWORD environment variables is not set, using defalut(admin/admin).${NC}"
    fi
    #新建 flag 标记文件
    touch /etc/kvmd/.init_flag
fi

#尝试挂载 usb_gadget
if [ "$OTG" == "1" ]; then
    echo "Trying OTG Port..."
    if [ -d /sys/kernel/config/usb_gadget/kvmd ]; then
        echo -e "${RED}Usb_gadget kvmd exists, please reboot your host system. ${NC}"
        exit -1
    elif [ ! -d /sys/kernel/config/usb_gadget ]; then
        mount -t configfs none /sys/kernel/config
    fi
fi

echo -e "${GREEN}One-KVM starting...${NC}"
exec supervisord -c /etc/kvmd/supervisord.conf