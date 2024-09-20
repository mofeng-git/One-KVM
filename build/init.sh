#!/bin/bash

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}One-KVM pre-starting...${NC}"

#仅首次运行，用于初始化配置文件
if [ ! -f /etc/kvmd/.init_flag ]; then

    mkdir -p /etc/kvmd/
    mv /etc/kvmd_backup/* /etc/kvmd/
    touch /etc/kvmd/.docker_flag

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

    
    if [ "$NOAUTH" == "1" ]; then
        sed -i "s/enabled: true/enabled: false/g" /etc/kvmd/override.yaml
    fi

    #生成 supervisord 配置文件是否添加扩展服务
    if [ "$NOWEBTERM" == "1" ]; then
        echo -e "${GREEN}One-KVM webterm is disabled.${NC}"
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
    fi

    #if [ ! -z "$SHUTDOWNPIN"  ! -z "$REBOOTPIN" ]; then

    #/dev/kvmd-video 设备优先级高于 /dev/video0,/dev/kvmd-hid 设备优先级高于 /dev/ttyUSB0
    if [ -f /dev/kvmd-video ] && [ -z "$VIDEONUM" ]; then
        echo -e "${GREEN}Found dev/kvmd-video, use it as kvmd video device.${NC}"
        sed -i "s/\/dev\/video0/\/dev\/kvmd-video/g" /etc/kvmd/override.yaml
    fi
    if [ ! -z "$VIDEONUM" ]; then
        sed -i "s/\/dev\/video0/\/dev\/video$VIDEONUM/g" /etc/kvmd/override.yaml
    fi
    if [ -f /dev/kvmd-hid ]; then
        echo -e "${GREEN}Found /dev/kvmd-hid, use it as kvmd video device.${NC}"
        sed -i "s/\/dev\/ttyUSB0/\/dev\/kvmd-hid/g" /etc/kvmd/override.yaml
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
    python -m kvmd.apps.otg start
fi

echo -e "${GREEN}One-KVM starting...${NC}"
exec supervisord -c /etc/kvmd/supervisord.conf