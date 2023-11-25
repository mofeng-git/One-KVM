PYVER=$(python3 -V)
ARCH=$(uname -m)
CURRENTWD=$PWD
echo $PYVER
echo $ARCH

if [[ "$PYVER" != *"3.10"* && $(which python3.10) != *"python"* ]]; then
  echo "你似乎没有安装 Python 3.10！" 
fi

mv ./patch/meson8b-onecloud.dtb /boot/dtb/meson8b-onecloud.dtb && echo "设备树文件覆盖成功！"
gzip -dc ./patch/Boot_SkipUSBBurning.gz | dd of=/dev/mmcblk1 && echo "覆盖引导成功！"

bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) --source mirrors.tuna.tsinghua.edu.cn --updata-software false --web-protocol http && echo "换源成功！"
apt install -y nginx tesseract-ocr tesseract-ocr-eng janus libevent-dev libgpiod-dev tesseract-ocr-chi-sim
echo "正在安装PiKVM......"  
dgkg -i ./fruity-pikvm_0.2_armhf.deb && echo "PiKVM安装成功！" && systemctl enable kvmd-vnc

mv ./patch/chinese.patch /usr/share/kvmd/web/ && cd /usr/share/kvmd/web/ && patch -p0 < chinese.patch
mv ./patch/3.198msd.patch /usr/local/lib/python3.10/kvmd-packages/ && cd /usr/local/lib/python3.10/kvmd-packages/ && patch -s -p0 < 3.198msd.patch
echo "补丁应用成功！"

cd $CURRENTWD && mv ./patch/long_press_gpio420 /usr/bin && mv ./patch/short_press_gpio420 /usr/bin && echo "GPIO-420脚本移动成功！"
mv ./config/main.yaml /etc/kvmd/ && mv ./config/override.yaml /etc/kvmd/ && echo "配置文件修改成功！"

kvmd -m && echo "请给玩客云重新上电，然后就可以开始使用One-KVM了！"


