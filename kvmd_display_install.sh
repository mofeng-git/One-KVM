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
ExecStart=/usr/share/kvmd/display_when_ustream_exists.sh
TimeoutStopSec=10
KillMode=mixed

[Install]
WantedBy=multi-user.target
EOF
  cp -f ./patch/stream.sh /usr/share/kvmd/ && cp -f ./patch/stream_when_ustream_exists.sh /usr/share/kvmd/ && chmod +x /usr/share/kvmd/stream.sh /usr/share/kvmd/stream_when_ustream_exists.sh
  #启动服务
  systemctl enable kvmd-display && systemctl start kvmd-display
}

kvmd_display