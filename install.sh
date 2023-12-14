PYVER=$(python3 -V)
ARCH=$(uname -m)
CURRENTWD=$PWD
echo $PYVER
echo $ARCH

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


if [[ "$PYVER" != *"3.10"* && $(which python3.10) != *"python"* ]]; then
  echo "您似乎没有安装 Python 3.10！" 
  exit
else
  update-alternative
fi

cp ./patch/meson8b-onecloud.dtb /boot/dtb/meson8b-onecloud.dtb && echo "设备树文件覆盖成功！"

if [ -f "./installed.txt" ]; then
  rm /etc/kvmd/nginx/ssl/server.crt
  rm /etc/kvmd/nginx/ssl/server.key
else
  #此为危险操作，会覆盖MBR分区，请在没有自行分区前执行，否则会丢失分区数据系统无法启动！  
  gzip -dc ./patch/Boot_SkipUSBBurning.gz | dd of=/dev/mmcblk1 && echo "One-KVM V0.4" >> installed.txt && echo "覆盖引导成功！"
  echo kvmd ALL=\(ALL\) NOPASSWD: /usr/bin/long_press_gpio420,/usr/bin/short_press_gpio420 >>  /etc/sudoers
fi



if [ -f "./installed.txt" ]; then
  echo "您似乎已经安装fruity-pikvm_0.2_armhf.deb，是否覆盖安装,取消此操作不会影响后续文件修改操作（Y/N）？"
  read USERYN
  case $USERYN in 
    N | n)
      echo "跳过安装fruity-pikvm_0.2_armhf.deb！"
    ;;
    *)
      echo "正在安装PiKVM......"  
      dpkg -i ./fruity-pikvm_0.2_armhf.deb >> ./log.txt &&  systemctl enable kvmd-vnc && echo "PiKVM安装成功！" 
    ;;
  esac
else
  bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) --source mirrors.tuna.tsinghua.edu.cn --updata-software false --web-protocol http && echo "换源成功！"
  echo "正在安装依赖软件nginx tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev tesseract-ocr-chi-sim......"  
  apt install -y nginx tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev tesseract-ocr-chi-sim  >> ./log.txt
  echo "正在安装PiKVM......"  
  dpkg -i ./fruity-pikvm_0.2_armhf.deb >> ./log.txt &&  systemctl enable kvmd-vnc && echo "PiKVM安装成功！" 
  cd $CURRENTWD
  cp ./patch/chinese.patch /usr/share/kvmd/web/ && cd /usr/share/kvmd/web/ && patch -s -p0 < chinese.patch
  cd $CURRENTWD
  cp ./patch/3.198msd.patch /usr/local/lib/python3.10/kvmd-packages/ && cd /usr/local/lib/python3.10/kvmd-packages/ && patch -s -p0 < 3.198msd.patch
  echo "补丁应用成功！"
fi

cd $CURRENTWD && cp -f ./patch/long_press_gpio420 /usr/bin && cp -f ./patch/short_press_gpio420 /usr/bin && chmod +x /usr/bin/long_press_gpio420 && chmod +x /usr/bin/short_press_gpio420 && echo "GPIO-420脚本移动成功！"
cp -f ./patch/hw.py /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/ && chmod +x /usr/local/lib/python3.10/kvmd-packages/kvmd/apps/kvmd/info/hw.py
cp -f ./config/main.yaml /etc/kvmd/ && cp -f ./config/override.yaml /etc/kvmd/ && echo "文件修改成功！"

if [ -f "./installed.txt" ]; then
  kvmd -m >> ./log.txt
  echo "机器已执行重启命令，稍作等待就可以开始使用One-KVM了！"
else
  kvmd -m >> ./log.txt
  echo "机器已执行重启命令，请手动给玩客云重新上电（拔插电源），然后就可以开始使用One-KVM了！"
fi

ipaddr=`ip addr | grep "scope global" | awk '{print $2}' |awk -F/ '{print $1}'`
echo -e "内网访问地址为：\nhttp://$ipaddr\nhttps://$ipaddr"
reboot