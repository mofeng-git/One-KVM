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
    echo -e "\e[0;31m此脚本暂不支持armv7l架构以外的设备！\n退出脚本！" 
    exit
  fi

  if [[ "$PYVER" != *"3.10"* && $(which python3.10) != *"python"* ]]; then
    echo -e "您似乎没有安装 Python 3.10！\n退出脚本！\e[0;37m" 
    exit
  fi
}

#安装依赖软件
install-dependencies(){
  bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) --source mirrors.tuna.tsinghua.edu.cn --updata-software false --web-protocol http && echo "换源成功！"
  echo -e "\e[0;32m正在安装依赖软件p......"  
  apt install -y python3.10 python3-pip python3-dev patch iptables nginx \
    tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev \
    tesseract-ocr-chi-sim  libjpeg-dev libfreetype6-dev
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
  cp -f ./patch/onecloud_gpio.sh /usr/bin
  chmod +x /usr/bin/onecloud_gpio.sh 
  echo "GPIO脚本移动成功！"
  cp -f ./patch/hw.py /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/
  chmod +x /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/hw.py
  cp -f ./config/main.yaml /etc/kvmd/ && cp -f ./config/override.yaml /etc/kvmd/ 
  echo "配置文件替换成功！"
  kvmd -m 
}

#应用补丁
add-patches(){
  if [ ! -f `grep -c "$FIND_STR" $FIND_FILE`  ]; then
    echo kvmd ALL=\(ALL\) NOPASSWD: /usr/bin/onecloud_gpio.sh >>  /etc/sudoers
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

printf "    欢迎使用 One-KVM - 基于开源程序PiKVM的IP-KVM 应用
    ____________________________________________________________________________

    要修改默认账户（admin）密码可使用 \"kvmd-htpasswd set admin\"

    帮助链接：
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
}

check-environment
install-dependencies
install-pikvm
add-patches
fix-motd
show-info