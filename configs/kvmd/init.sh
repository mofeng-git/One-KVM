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
         echo -e "${GREEN}One-KVM SSL is disabled.${NC}"
        python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf  -o nginx/https/enabled=false
    else
        python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf
    fi

    #生成 supervisord 配置文件是否添加扩展服务

    if [ "$NOAUTH" == "1" ]; then
        sed -i "s/enabled: true/enabled: false/g" /etc/kvmd/override.yaml
    fi

    if [ "$NOWEBTERMWRITE" == "1" ]; then
        WEBTERMWRITE == ""
    else
        WEBTERMWRITE == "-W"
    fi

    if [ "$NOWEBTERM" == "1" ]; then
        echo -e "${GREEN}One-KVM webterm is disabled.${NC}"
        rm -r /usr/share/kvmd/extras/webterm
    else
        cat >> /etc/kvmd/supervisord.conf  << EOF

[program:kvmd-webterm]
command=/usr/local/bin/ttyd --interface=/run/kvmd/ttyd.sock --port=0 $WEBTERMWRITE /bin/bash -c '/etc/kvmd/armbain-motd; bash'
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

    if [  "&NOVNC" == "1" ]; then
        echo -e "${GREEN}One-KVM VNC is disabled.${NC}"
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
        echo -e "${GREEN}One-KVM IPMI is disabled.${NC}"
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