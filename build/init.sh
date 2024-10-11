#!/bin/bash

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}One-KVM pre-starting...${NC}"

if [ ! -f /etc/kvmd/.init_flag ]; then
    echo -e "${GREEN}One-KVM is initializing first...${NC}" \
        && mkdir -p /etc/kvmd/ \
        && mv /etc/kvmd_backup/* /etc/kvmd/ \
        && touch /etc/kvmd/.docker_flag \
        && sed -i 's/localhost.localdomain/docker/g' /etc/kvmd/meta.yaml \
        && sed -i 's/localhost/localhost:4430/g' /etc/kvmd/kvm_input.sh \
        && /usr/share/kvmd/kvmd-gencert --do-the-thing \
        && /usr/share/kvmd/kvmd-gencert --do-the-thing --vnc \
        || echo -e "${RED}One-KVM config moving and self-signed SSL certificates init failed.${NC}"
   
    if [ "$NOSSL" == 1 ]; then
        echo -e "${GREEN}One-KVM self-signed SSL is disabled.${NC}" \
        && python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf  -o nginx/https/enabled=false \
        || echo -e "${RED}One-KVM nginx config init failed.${NC}"
    else
        python -m kvmd.apps.ngxmkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf \
        || echo -e "${RED}One-KVM nginx config init failed.${NC}"
    fi
   
    if [ "$NOAUTH" == "1" ]; then
        sed -i "s/enabled: true/enabled: false/g" /etc/kvmd/override.yaml \
        && echo -e "${GREEN}One-KVM auth is disabled.${NC}"
    fi

    #add supervisord conf
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

    #switch OTG config
    if [ "$OTG" == "1" ]; then
        echo -e "${GREEN}One-KVM OTG is enabled.${NC}"
        sed -i "s/ch9329/otg/g" /etc/kvmd/override.yaml
	    sed -i "s/device: \/dev\/ttyUSB0//g" /etc/kvmd/override.yaml
        if [ "$NOMSD" == 1 ]; then
            echo -e "${GREEN}One-KVM MSD is disabled.${NC}"
        else
            sed -i "s/#type: otg/type: otg/g" /etc/kvmd/override.yaml
        fi
    fi

    #if [ ! -z "$SHUTDOWNPIN"  ! -z "$REBOOTPIN" ]; then

    if [ ! -z "$VIDEONUM" ]; then
        sed -i "s/\/dev\/video0/\/dev\/video$VIDEONUM/g" /etc/kvmd/override.yaml \
            && echo -e "${GREEN}One-KVM video device is set to /dev/video$VIDEONUM.${NC}"
    fi

    #set htpasswd
    if [ ! -z "$USERNAME" ] && [ ! -z "$PASSWORD" ]; then
        python -m kvmd.apps.htpasswd del admin \
            && echo $PASSWORD | python -m kvmd.apps.htpasswd set -i  "$USERNAME" \
            && echo "$PASSWORD -> $USERNAME:$PASSWORD" > /etc/kvmd/vncpasswd \
            && echo "$USERNAME:$PASSWORD -> $USERNAME:$PASSWORD" > /etc/kvmd/ipmipasswd \
            || echo -e "${RED}One-KVM htpasswd init failed.${NC}"
    else
        echo -e "${YELLOW} USERNAME and PASSWORD environment variables is not set, using defalut(admin/admin).${NC}"
    fi
    
    touch /etc/kvmd/.init_flag
fi


#Trying usb_gadget
if [ "$OTG" == "1" ]; then
    echo "Trying OTG Port..."
    rm -r /run/kvmd/otg &> /dev/null
    modprobe libcomposite || echo -e "${RED}Linux libcomposite module modprobe failed.${NC}"
    python -m kvmd.apps.otg start \
        && ln -s /dev/hidg1 /dev/kvmd-hid-mouse \
        && ln -s /dev/hidg0 /dev/kvmd-hid-keyboard \
        || echo -e "${RED}OTG Port mount failed.${NC}"
fi

echo -e "${GREEN}One-KVM starting...${NC}"
exec supervisord -c /etc/kvmd/supervisord.conf