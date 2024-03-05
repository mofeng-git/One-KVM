#!/bin/bash

ARCH=$(uname -m)
MACHINE=$(uname -o -s -r -m)
PYVER=$(python3 -V)
CURRENTWD=$PWD
FIND_FILE="/etc/sudoers"
FIND_STR="short_press_gpio420"

#检查架构和Python版本
check-environment(){
  echo -e "\e[0;32m设备名称：$MACHINE\nPython版本：$PYVER"
  if [ ! $ARCH = "armv7l" ]; then
    echo -e "\e[0;31m暂不支持$MACHINE架构以外的设备！\n退出脚本！" 
    exit
  fi

  if [[ "$PYVER" != *"3.10"* && $(which python3.10) != *"python"* ]]; then
    echo -e "您似乎没有安装 Python 3.10！\n退出脚本！\e[0;37m" 
    exit
  else
    update-alternative
  fi
}

#使用Python3.10版本
update-alternative(){
  counter=2
  for i in {1..9}
  do
    bindir=$(which python3.$i)
    if [[ $bindir == *"bin"* ]]; then
      echo $i $bindir
      update-alternatives --install /usr/bin/python3 python3 $bindir $counter
      let counter++
    fi
  done
  update-alternatives --install /usr/bin/python3 python3 $(which python3.10) 1
  update-alternatives --set python3 $(which python3.10)
}

#修改设备树文件
change-device-tree(){
  cp -f ./patch/meson8b-onecloud.dtb /boot/dtb/meson8b-onecloud.dtb
  echo "设备树文件覆盖成功！"
}

#覆盖引导分区
override-uboot(){
  echo -e "\e[0;31m玩客云默认启用USB线刷检测，是否保存原样？（\e[1;32mY保持原样/N关闭此功能）"
  read USERYN
  case $USERYN in 
    N | n)
      gzip -dc ./patch/Boot_SkipUSBBurning.gz | dd of=/dev/mmcblk1
      echo -e "\e[0;30m覆盖引导成功！\e[0;37m"
    ;;
    *)
      echo -e "\e[0;30m已跳过覆盖UBoot分区！\e[0;37m" 
    ;;
  esac
}

#安装依赖软件
install-dependencies(){
  bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) --source mirrors.tuna.tsinghua.edu.cn --updata-software false --web-protocol http && echo "换源成功！"
  echo -e "\e[0;32m正在安装依赖软件python3.10 patch iptables nginx tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev tesseract-ocr-chi-sim libjpeg-dev libfreetype6-dev......"  
  apt install -y python3.10 python3-pip python3-dev patch iptables nginx tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev tesseract-ocr-chi-sim  libjpeg-dev libfreetype6-dev armbian-config
}

#安装PiKVM
install-pikvm(){
  echo "正在安装PiKVM......"  
  dpkg -i ./fruity-pikvm_0.2_armhf.deb 
  systemctl enable kvmd-vnc
  systemctl disable nginx kvmd-janus
  #rm -f /lib/systemd/system/nginx.service 
  #rm -f /lib/systemd/system/kvmd-janus.service &&  systemctl daemon-reload
  echo "PiKVM安装成功！"
  cd $CURRENTWD
  cp -f ./patch/long_press_gpio420 /usr/bin && cp -f ./patch/short_press_gpio420 /usr/bin
  chmod +x /usr/bin/long_press_gpio420 && chmod +x /usr/bin/short_press_gpio420
  echo "GPIO-420脚本移动成功！"
  cp -f ./patch/hw.py /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/
  chmod +x /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/hw.py
  cp -f ./config/main.yaml /etc/kvmd/ && cp -f ./config/override.yaml /etc/kvmd/ 
  echo "配置文件替换成功！"
  kvmd -m 
}

#应用补丁
add-patches(){
  if [ ! -f `grep -c "$FIND_STR" $FIND_FILE`  ]; then
    echo kvmd ALL=\(ALL\) NOPASSWD: /usr/bin/long_press_gpio420,/usr/bin/short_press_gpio420 >>  /etc/sudoers
  fi

  if [ ! -f "/usr/local/lib/python3.10/kvmd-packages/3.198msd.patch"  ]; then
    cd $CURRENTWD
    cp ./patch/3.198msd.patch /usr/local/lib/python3.10/kvmd-packages/ && cd /usr/local/lib/python3.10/kvmd-packages/
    patch -s -p0 < 3.198msd.patch
    echo "MSD补丁应用成功！"
  fi

  cd $CURRENTWD
  cp -f ./patch/chinese.patch /usr/share/kvmd/web/ && cd /usr/share/kvmd/web/
  patch -s -p0 < chinese.patch
  echo  -e "\e[0;32m中文补丁应用成功！"
  pip3 config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple/
  pip3 install -U Pillow

}

fix-motd() {
	#cd $CURRENTWD
  if [ -e /etc/motd ]; then rm /etc/motd; fi
  cat > /usr/bin/armbian-motd << EOF
#!/bin/sh
if [ -e /etc/update-motd.d/10-armbian-header ]; then /etc/update-motd.d/10-armbian-header; fi
if [ -e /etc/update-motd.d/30-armbian-sysinfo ]; then /etc/update-motd.d/30-armbian-sysinfo; fi

printf "    Welcome to One-KVM - Open Source IP-KVM installed on onecloud board
    ____________________________________________________________________________

    To prevent kernel messages from printing to the terminal use \"dmesg -n 1\".

    To change KVM password use command \"kvmd-htpasswd set admin\".

    Useful links:
      * https://pikvm.org
      * https://docs.pikvm.org
      * https://github.com/mofeng-git/One-KVM

"
EOF
	chmod +x /usr/bin/armbian-motd  /etc/update-motd.d/10-armbian-header /etc/update-motd.d/30-armbian-sysinfo
  sed -i 's/cat \/etc\/motd/armbian-motd/g' /lib/systemd/system/kvmd-webterm.service
	echo "fixed motd"
}

show-info(){
  ipaddr=`ip addr | grep "scope global" | awk '{print $2}' |awk -F/ '{print $1}'`
  echo  -e "\e[0;32m内网访问地址为：\nhttp://$ipaddr\nhttps://$ipaddr"
  echo "机器已重启，等待10秒然后拔插电源，One-KVM就安装完成了！"
}

#配置H.264功能
kvmd-ffmpeg-h-264(){
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
  sed -i '293c \/\/' /usr/share/kvmd/web/share/js/kvm/stream_janus.js
  sed -i 's/request\": \"watch\", \"p/request\": \"watch\", \"id\" : 1, \"p/' /usr/share/kvmd/web/share/js/kvm/stream_janus.js
  #补全网页JS文件并添加相应脚本
  mkdir /usr/share/janus/javascript/ && cp -f ./web/adapter.js /usr/share/janus/javascript/ && cp -f ./web/janus.js /usr/share/janus/javascript/
  cp -f ./patch/stream.sh /usr/share/kvmd/ && cp -f ./patch/stream_when_ustream_exists.sh /usr/share/kvmd/ && chmod +x /usr/share/kvmd/stream.sh /usr/share/kvmd/stream_when_ustream_exists.sh
  #启动服务
  #systemctl enable kvmd-ffmpeg && systemctl enable kvmd-janus-static
  #systemctl start kvmd-ffmpeg && systemctl start kvmd-janus-static
}


check-environment

#Only for onecloud Armbian with kernel 5.10,now this these two steps is deprecated!
#override-uboot
#change-device-tree

install-dependencies
install-pikvm
add-patches
fix-motd

#H.264 soft encoded video, default off, uncomment if needed
#kvmd-ffmpeg-h-264
show-info
reboot
