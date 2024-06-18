#!/bin/bash

CURRENTWD=$PWD

#配置H.264功能
kvmd_ffmpeg_h-264(){
  echo "正在配置H.264功能..."
  cd $CURRENTWD
  apt install -y ffmpeg
  #写入ffmpeg转码推流文件和janus streaming配置文件
  cp -r /etc/kvmd/janus /etc/kvmd/janus2
  rm /etc/kvmd/janus2/janus.plugin.ustreamer.jcfg
  cat > /etc/kvmd/janus2/janus.plugin.streaming.jcfg << EOF
kvmd-ffmpeg: {
        type = "rtp"
        id = 1
        description = "H.264 live stream coming from ustreamer"
        audio = false
        video = true
        videoport = 5004
        videopt = 96
        videocodec = "h264"
        videofmtp = "profile-level-id=42e01f;packetization-mode=1"
        videortpmap = "H264/90000"
}
EOF

  cat > /lib/systemd/system/kvmd-ffmpeg.service << EOF
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
ExecStart=/usr/share/kvmd/stream_when_ustream_exists.sh
TimeoutStopSec=10
KillMode=mixed

[Install]
WantedBy=multi-user.target
EOF
  #修改原有kvmd代码和配置文件
  sed -i '17s/.*/ExecStart=\/usr\/bin\/janus --disable-colors --configs-folder=\/etc\/kvmd\/janus2/' /lib/systemd/system/kvmd-janus-static.service
  sed -i 's/janus.plugin.ustreamer/janus.plugin.streaming/' /usr/share/kvmd/web/share/js/kvm/stream_janus.js
  sed -i '324c \/\/' /usr/share/kvmd/web/share/js/kvm/stream_janus.js
  sed -i 's/request\": \"watch\", \"p/request\": \"watch\", \"id\" : 1, \"p/' /usr/share/kvmd/web/share/js/kvm/stream_janus.js
  #补全网页JS文件并添加相应脚本
  if [ ! -e /usr/share/janus/javascript/adapter.js ]; then 
    mkdir /usr/share/janus/javascript/
    cp -f ./patches/adapter.js /usr/share/janus/javascript/ && cp -f ./patches/janus.js /usr/share/janus/javascript/
  fi
  cp -f ./patches/stream.sh /usr/share/kvmd/ && cp -f ./patches/stream_when_ustream_exists.sh /usr/share/kvmd/ && chmod +x /usr/share/kvmd/stream.sh /usr/share/kvmd/stream_when_ustream_exists.sh
  #启动服务
  systemctl daemon-reload
  ! $NOTCHROOT || systemctl enable kvmd-ffmpeg && systemctl enable kvmd-janus-static
  ! $NOTCHROOT || systemctl start kvmd-ffmpeg && systemctl start kvmd-janus-static
  echo "配置完成"
}

kvmd_ffmpeg_h-264