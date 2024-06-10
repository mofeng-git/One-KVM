#!/bin/bash

ARCH=$(uname -n)
MACHINE=$(uname -o -s -r -m)
PYVER=$(python3 -V)
CURRENTWD=$PWD
FIND_FILE="/etc/sudoers"
FIND_STR="onecloud_gpio.sh"

#检查架构和Python版本
check_environment(){
  echo -e "设备名称：$MACHINE\nPython版本：$PYVER"
  if [ ! $ARCH = "onecloud" ]; then
    echo -e "此脚本暂不支持armv7l架构以外的设备！\n退出脚本！" 
    exit
  fi

  if [[ "$PYVER" != *"3.10"* && $(which python3.10) != *"python"* ]]; then
    echo -e "您似乎没有安装 Python 3.10！\n退出脚本！" 
    exit
  fi
}

#安装依赖软件
install_dependencies(){
  bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) --source mirrors.tuna.tsinghua.edu.cn --updata-software false --web-protocol http && echo "换源成功！"
  echo -e "正在安装依赖软件......"  
  apt install -y python3.10 python3-pip python3-dev patch iptables nginx \
    tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev \
    tesseract-ocr-chi-sim  libjpeg-dev libfreetype6-dev gcc
}

#安装PiKVM
install_pikvm(){
  echo "正在安装PiKVM......"  
  dpkg -i ./fruity-pikvm_0.2_armhf.deb 
  systemctl enable kvmd-vnc
  systemctl disable nginx kvmd-janus
  #rm -f /lib/systemd/system/nginx.service 
  #rm -f /lib/systemd/system/kvmd-janus.service &&  systemctl daemon-reload
  echo "PiKVM安装成功"
  cd $CURRENTWD
  cp -f ./patch/onecloud_gpio.sh /usr/bin
  chmod +x /usr/bin/onecloud_gpio.sh 
  echo "GPIO脚本移动成功"
  cp -f ./patch/hw.py /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/
  chmod +x /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/hw.py
  cp -f ./config/main.yaml /etc/kvmd/ && cp -f ./config/override.yaml /etc/kvmd/ 
  echo "配置文件替换成功"
  kvmd -m 
}

#应用补丁
add_patches(){
  if [ ! -f `grep -c "$FIND_STR" $FIND_FILE`  ]; then
    echo kvmd ALL=\(ALL\) NOPASSWD: /usr/bin/onecloud_gpio.sh >>  /etc/sudoers
  fi

  if [ ! -f "/usr/local/lib/python3.10/kvmd-packages/3.198msd.patch"  ]; then
    cd $CURRENTWD
    cp ./patch/3.198msd.patch /usr/local/lib/python3.10/kvmd-packages/ && cd /usr/local/lib/python3.10/kvmd-packages/
    patch -s -p0 < 3.198msd.patch
    echo "MSD补丁应用成功"
  fi

  cd $CURRENTWD
  cp -f ./patch/chinese.patch /usr/share/kvmd/web/ && cd /usr/share/kvmd/web/
  patch -s -p0 < chinese.patch
  echo  -e "中文补丁应用成功"
  pip3 config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple/
  pip3 install -U Pillow

}

#设置网页终端欢迎语
fix_motd(){
	#cd $CURRENTWD
  if [ -e /etc/motd ]; then rm /etc/motd; fi
  cat > /usr/bin/armbian-motd << EOF
#!/bin/sh
if [ -e /etc/update-motd.d/10-armbian-header ]; then /etc/update-motd.d/10-armbian-header; fi
if [ -e /etc/update-motd.d/30-armbian-sysinfo ]; then /etc/update-motd.d/30-armbian-sysinfo; fi

printf "    欢迎使用 One-KVM，基于开源程序 PiKVM 的 IP-KVM 应用
    ____________________________________________________________________________

    要修改默认账户 admin 密码可使用 \"kvmd-htpasswd set admin\"

    帮助链接：
      * https://docs.pikvm.org
      * https://one-kvm.mofeng.run/
      * https://github.com/mofeng-git/One-KVM
"
EOF
	chmod +x /usr/bin/armbian-motd  /etc/update-motd.d/10-armbian-header /etc/update-motd.d/30-armbian-sysinfo
  sed -i 's/cat \/etc\/motd/armbian-motd/g' /lib/systemd/system/kvmd-webterm.service
	echo "fixed motd"
}

#玩客云特定配置
onecloud_conf(){
  if [ ! $ARCH = "onecloud" ]; then
    echo -e "\n"
  else
    echo "为玩客云配置开机脚本"
    cat <<EOF >/etc/rc.local
#!/bin/bash
echo "default-on" >/sys/class/leds/onecloud\:green\:alive/trigger
echo "none" >/sys/class/leds/onecloud\:red\:alive/trigger
echo "none" >/sys/class/leds/onecloud\:blue\:alive/trigger
cpufreq-set -d 1200MHz -u 1200MHz
echo device > /sys/class/usb_role/c9040000.usb-role-switch/role
systemctl disable kvmd
systemctl start kvmd
exit 0
EOF
    #如果在CHROOT环境需设置NOTCHROOT=false
    ! $NOTCHROOT || gzip -dc ./patch/Boot_SkipUSBBurning.gz | dd of=/dev/mmcblk1 bs=512 seek=1 count=32767
    echo -e "\n"
  fi
}

#打印完成信息
show_info(){
  echo  -e "安装结束，重启之后即可开始使用One-KVM"
  /usr/bin/armbian-motd
}

check_environment
install_dependencies
install_pikvm
add_patches
fix_motd
onecloud_conf
show_info