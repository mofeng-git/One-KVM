#!/bin/bash

CURRENTWD=$PWD

#配置采集卡简单环出功能
kvmd_display(){
  echo "正在配置采集卡简单环出功能功能..."
  cd $CURRENTWD
  apt install -y ffmpeg
  cat > /lib/systemd/system/kvmd-display.service << EOF
[Unit]
Description=PiKVM - Transcode (Static Config)
After=network.target network-online.target nss-lookup.target kvmd.service

[Service]
User=kvmd
Group=kvmd
Type=simple
Restart=on-failure
RestartSec=3
AmbientCapabilities=CAP_NET_RAW
LimitNOFILE=65536
UMask=0117
ExecStart=/usr/share/kvmd/display.sh
TimeoutStopSec=10
KillMode=mixed

[Install]
WantedBy=multi-user.target
EOF
  cp -f ./patches/display.sh /usr/share/kvmd/ && chmod +x /usr/share/kvmd/display.sh
  #启动服务
  systemctl daemon-reload
  ! $NOTCHROOT || systemctl enable kvmd-display
  ! $NOTCHROOT || systemctl start kvmd-display
  echo "配置完成"
}

kvmd_display