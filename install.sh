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
fi
if [[ "$PYVER" != *"3.10"*  &&  $(which python3.10) == *"python"* ]]; then
  update-alternative
fi

cp ./patch/meson8b-onecloud.dtb /boot/dtb/meson8b-onecloud.dtb && echo "设备树文件覆盖成功！"
#此为危险操作，会覆盖MBR分区，请在没有自行分区前执行，否则会丢失分区数据导致挂载了EMMC其他分区的系统无法启动！
if [ -f "./installed.txt" ]; then
#防止新证书覆盖失败
    rm /etc/kvmd/nginx/ssl/server.crt
    rm /etc/kvmd/nginx/ssl/server.key
    echo "跳过覆盖引导！"
else
    gzip -dc ./patch/Boot_SkipUSBBurning.gz | dd of=/dev/mmcblk1 && echo "One-KVM V0.4" >> installed.txt && echo "覆盖引导成功！"
    
fi


bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) --source mirrors.tuna.tsinghua.edu.cn --updata-software false --web-protocol http && echo "换源成功！"
echo "正在安装依赖软件nginx tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev tesseract-ocr-chi-sim......"  
apt install -y nginx tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev tesseract-ocr-chi-sim  >> ./log.txt
echo "正在安装PiKVM......"  
dpkg -i ./fruity-pikvm_0.2_armhf.deb && echo "PiKVM安装成功！" && systemctl enable kvmd-vnc
cd $CURRENTWD
cp ./patch/chinese.patch /usr/share/kvmd/web/ && cd /usr/share/kvmd/web/ && patch -s -p0 < chinese.patch
cd $CURRENTWD
cp ./patch/3.198msd.patch /usr/local/lib/python3.10/kvmd-packages/ && cd /usr/local/lib/python3.10/kvmd-packages/ && patch -s -p0 < 3.198msd.patch
echo "补丁应用成功！"

cd $CURRENTWD && cp -f ./patch/long_press_gpio420 /usr/bin && cp -f ./patch/short_press_gpio420 /usr/bin && echo "GPIO-420脚本移动成功！"
cp -f ./config/main.yaml /etc/kvmd/ && cp -f ./config/override.yaml /etc/kvmd/ && echo "配置文件修改成功！"

kvmd -m >> ./log.txt
if [ -f "./installed.txt" ]; then
    echo "机器已执行重启命令，重启成功后就可以开始使用One-KVM了！"
    echo "如果已经挂载了MSD分区，请手动编辑/etc/kvmd/override.yaml修改msd选项为otg。"
else
    echo "机器已执行重启命令，请手动给玩客云重新上电（拔插电源），然后就可以开始使用One-KVM了！"
fi
reboot

