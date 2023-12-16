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

#删除SSL证书
delete-ssl(){
  if [ -f "/etc/kvmd/nginx/ssl/server.crt" ] || [ -f "/etc/kvmd/nginx/ssl/server.key" ]; then
    echo -e "正在删除SSL证书！\e[0;37m"
    rm /etc/kvmd/nginx/ssl/server.crt
    rm /etc/kvmd/nginx/ssl/server.key
  fi  
}

#修改设备树文件
change-device-tree(){
  cp -f./patch/meson8b-onecloud.dtb /boot/dtb/meson8b-onecloud.dtb
  echo "设备树文件覆盖成功！"
}

#覆盖引导分区
override-uboot(){
  echo -e "\e[0;31m是否选择跳过按下重置键时的玩客云USB线刷检测？（\e[1;31mY/\e[1;32mN\e[0;31m）"
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
  echo "\e[0;32m正在安装依赖软件nginx tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev tesseract-ocr-chi-sim......"  
  apt install -y nginx tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev tesseract-ocr-chi-sim  >> ./log.txt
}

#安装PiKVM
install-pikvm(){
  delete-ssl
  echo "正在安装PiKVM......"  
  dpkg -i ./fruity-pikvm_0.2_armhf.deb >> ./log.txt
  systemctl enable kvmd-vnc
  echo "PiKVM安装成功！"
  cd $CURRENTWD
  cp -f ./patch/long_press_gpio420 /usr/bin && cp -f ./patch/short_press_gpio420 /usr/bin
  chmod +x /usr/bin/long_press_gpio420 && chmod +x /usr/bin/short_press_gpio420
  echo "GPIO-420脚本移动成功！"
  cp -f ./patch/hw.py /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/
  chmod +x /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/hw.py
  cp -f ./config/main.yaml /etc/kvmd/ && cp -f ./config/override.yaml /etc/kvmd/ 
  echo "配置文件替换成功！"
  kvmd -m >> ./log.txt
}

#应用补丁
add-patches(){
  if [ -f `grep -c "$FIND_STR" $FIND_FILE` -ne '1' ]; then
    echo kvmd ALL=\(ALL\) NOPASSWD: /usr/bin/long_press_gpio420,/usr/bin/short_press_gpio420 >>  /etc/sudoers
  fi
  if[ ! -f /usr/local/lib/python3.10/kvmd-packages/3.198msd.patch ];then
    cd $CURRENTWD
    cp ./patch/3.198msd.patch /usr/local/lib/python3.10/kvmd-packages/ && cd /usr/local/lib/python3.10/kvmd-packages/ && patch -s -p0 < 3.198msd.patch
    echo "MSD补丁应用成功！"
  fi
  if[ ! -f /usr/local/lib/python3.10/kvmd-packages/chinese.patch ];then
    cd $CURRENTWD
    cp./patch/chinese.patch /usr/share/kvmd/web/ && cd /usr/share/kvmd/web/ && patch -s -p0 < chinese.patch
    echo "中文补丁应用成功！"
  else
    #此处还要先恢复中文补丁再应用新补丁
    echo "中文补丁应用成功！"
  fi
  echo -e "ENABLE=true\nMIN_SPEED=1536000\nMAX_SPEED=1536000\nGOVERNOR=performance" > /etc/default/cpufrequtils
  service cpufrequtils restart
}


show-info(){
  echo "One-KVM V0.5" >> installed.txt 
  ipaddr=`ip addr | grep "scope global" | awk '{print $2}' |awk -F/ '{print $1}'`
  echo -e "内网访问地址为：\nhttp://$ipaddr\nhttps://$ipaddr"
  echo "机器已重启，等待10秒然后拔插电源，One-KVM就安装完成了！"
}

if [ -f "./installed.txt" ]; then
  echo "\e[0;31m检测到存在安装One-KVM记录！"
else
  #此为危险操作，会覆盖MBR分区，请在没有自行分区前执行，否则会丢失分区数据系统无法启动！  
  
  
fi




check-environment
override-uboot
change-device-tree
install-dependencies
install-pikvm
add-patches
show-info
reboot