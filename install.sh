#!/bin/bash
# https://github.com/srepac/kvmd-armbian
# modified by SilentWind        2024-06-17
# modified by xe5700            2021-11-04      xe5700@outlook.com
# modified by NewbieOrange      2021-11-04
# created by @srepac   08/09/2021   srepac@kvmnerds.com
# Scripted Installer of Pi-KVM on Armbian 32-bit and 64-bit (as long as it's running python 3.10 or higher)
#
# *** MSD is disabled by default ***
#
# Mass Storage Device requires the use of a USB thumbdrive or SSD and will need to be added in /etc/fstab
: '
# SAMPLE /etc/fstab entry for USB drive with only one partition formatted as ext4 for the entire drive:

/dev/sda1  /var/lib/kvmd/msd   ext4  nodev,nosuid,noexec,ro,errors=remount-ro,data=journal,X-kvmd.otgmsd-root=/var/lib/kvmd/msd,X-kvmd.otgmsd-user=kvmd  0  0

'
# NOTE:  This was tested on a new install of raspbian desktop and lite versions, but should also work on an existing install.
#
# Last change 20240526 2345 PDT
VER=3.4
set +x
PIKVMREPO="https://files.pikvm.org/repos/arch/rpi4"
KVMDFILE="kvmd-3.291-1-any.pkg.tar.xz"
KVMDCACHE="/var/cache/kvmd"; mkdir -p $KVMDCACHE
PKGINFO="${KVMDCACHE}/packages.txt"
APP_PATH=$(readlink -f $(dirname $0))
LOGFILE="${KVMDCACHE}/installer.log"; touch $LOGFILE; echo "==== $( date ) ====" >> $LOGFILE

if [[ "$1" == "-h" || "$1" == "--help" ]]; then
  echo "usage:  $0 [-f]   where -f will force re-install new pikvm platform"
  exit 1
fi

CWD=`pwd`

WHOAMI=$( whoami )
if [ "$WHOAMI" != "root" ]; then
  echo "$WHOAMI, please run script as root."
  exit 1
fi

PYTHONVER=$( python3 -V | cut -d' ' -f2 | cut -d'.' -f1,2 )
case $PYTHONVER in
  3.10|3.11)
    echo "Python $PYTHONVER is supported." | tee -a $LOGFILE
    ;;
  *)
    echo "Python $PYTHONVER is NOT supported.  Please make sure you have python3.10 or higher installed.  Exiting." | tee -a $LOGFILE
    exit 1
    ;;
esac

MAKER=$(tr -d '\0' < /proc/device-tree/model | awk '{print $1}')


gen-ssl-certs() {
  cd /etc/kvmd/nginx/ssl
  openssl ecparam -out server.key -name prime256v1 -genkey
  openssl req -new -x509 -sha256 -nodes -key server.key -out server.crt -days 3650 \
        -subj "/C=US/ST=Denial/L=Denial/O=Pi-KVM/OU=Pi-KVM/CN=$(hostname)"
  cp server* /etc/kvmd/vnc/ssl/
  cd ${APP_PATH}
} # end gen-ssl-certs


create-override() {
  if [ $( grep ^kvmd: /etc/kvmd/override.yaml | wc -l ) -eq 0 ]; then

    if [[ $( echo $platform | grep usb | wc -l ) -eq 1 ]]; then
      cat <<USBOVERRIDE >> /etc/kvmd/override.yaml
kvmd:
    hid:
        mouse_alt:
            device: /dev/kvmd-hid-mouse-alt  # allow relative mouse mode
    msd:
        type: disabled
    atx:
        type: disabled
    streamer:
        #forever: true
        cmd_append:
            - "--slowdown"      # so target doesn't have to reboot
        resolution:
            default: 1280x720
USBOVERRIDE

    else

      cat <<CSIOVERRIDE >> /etc/kvmd/override.yaml
kvmd:
    ### disable fan socket check ###
    info:
        fan:
            unix: ''
    hid:
        mouse_alt:
            device: /dev/kvmd-hid-mouse-alt
    msd:
        type: disabled
    streamer:
        #forever: true
        cmd_append:
            - "--slowdown"      # so target doesn't have to reboot
CSIOVERRIDE

    fi

  fi
} # end create-override

install-python-packages() {
  echo "apt install -y python3-aiofiles python3-aiohttp python3-appdirs python3-asn1crypto python3-async-timeout
    python3-bottle python3-cffi python3-chardet python3-click python3-colorama python3-cryptography python3-dateutil
    python3-dbus python3-dev python3-hidapi python3-idna python3-libgpiod python3-mako python3-marshmallow
    python3-more-itertools python3-multidict python3-netifaces python3-packaging python3-passlib python3-pillow
    python3-ply python3-psutil python3-pycparser python3-pyelftools python3-pyghmi python3-pygments python3-pyparsing
    python3-requests python3-semantic-version python3-setproctitle python3-setuptools python3-six python3-spidev
    python3-systemd python3-tabulate python3-urllib3 python3-wrapt python3-xlib python3-yaml python3-yarl python3-build
    python3-pyotp python3-qrcode python3-serial"
  apt install -y python3-aiofiles python3-aiohttp python3-appdirs python3-asn1crypto python3-async-timeout\
    python3-bottle python3-cffi python3-chardet python3-click python3-colorama python3-cryptography python3-dateutil\
    python3-dbus python3-dev python3-hidapi python3-idna python3-libgpiod python3-mako python3-marshmallow\
    python3-more-itertools python3-multidict python3-netifaces python3-packaging python3-passlib python3-pillow\
    python3-ply python3-psutil python3-pycparser python3-pyelftools python3-pyghmi python3-pygments python3-pyparsing\
    python3-requests python3-semantic-version python3-setproctitle python3-setuptools python3-six python3-spidev\
    python3-systemd python3-tabulate python3-urllib3 python3-wrapt python3-xlib python3-yaml python3-yarl python3-build\
    python3-pyotp python3-qrcode python3-serial >> $LOGFILE
} # end install python-packages

otg-devices() {
  modprobe libcomposite
  if [ ! -e /sys/kernel/config/usb_gadget/kvmd ]; then
    mkdir -p /sys/kernel/config/usb_gadget/kvmd/functions
    cd /sys/kernel/config/usb_gadget/kvmd/functions
    mkdir hid.usb0  hid.usb1  hid.usb2  mass_storage.usb0
  fi
  cd ${APP_PATH}
} # end otg-device creation

boot-files() {
  # Remove OTG serial (Orange pi zero's kernel not support it)
  sed -i '/^g_serial/d' /etc/modules

  # /etc/modules required entries for DWC2, HID and I2C
  if [[ $( grep -w dwc2 /etc/modules | wc -l ) -eq 0 ]]; then
    echo "dwc2" >> /etc/modules
  fi
  if [[ $( grep -w libcomposite /etc/modules | wc -l ) -eq 0 ]]; then
    echo "libcomposite" >> /etc/modules
  fi
  if [[ $( grep -w i2c-dev /etc/modules | wc -l ) -eq 0 ]]; then
    echo "i2c-dev" >> /etc/modules
  fi

  printf "\n/etc/modules\n\n" | tee -a $LOGFILE
  cat /etc/modules | tee -a $LOGFILE
} # end of necessary boot files

get-packages() {
  printf "\n\n-> Getting Pi-KVM packages from ${PIKVMREPO}\n\n" | tee -a $LOGFILE
  cp -f ${APP_PATH}/kvmd-packages/* ${KVMDCACHE}

  } # end get-packages function

get-platform() {
    platform="kvmd-platform-v2-hdmiusb-rpi4"; 
    echo
    echo "Platform selected -> $platform" | tee -a $LOGFILE
    echo
} # end get-platform

install-kvmd-pkgs() {
  cd /

  INSTLOG="${KVMDCACHE}/installed_ver.txt"; rm -f $INSTLOG
  date > $INSTLOG

# uncompress platform package first
  i=$( ls ${KVMDCACHE}/${platform}*.tar.xz )  ### install the most up to date kvmd-platform package

  # change the log entry to show 3.291 platform installed as we'll be forcing kvmd-3.291 instead of latest/greatest kvmd
  _platformver=$( echo $i | sed -e 's/3\.29[2-9]*/3.291/g' -e 's/3\.3[0-9]*/3.291/g' -e 's/3.2911/3.291/g' -e 's/4\.[0-9].*-/3.291-/g' )
  echo "-> Extracting package $_platformver into /" | tee -a $INSTLOG
  tar xfJ $i

# then uncompress, kvmd-{version}, kvmd-webterm, and janus packages
  for i in $( ls ${KVMDCACHE}/*.tar.xz | egrep 'kvmd-[0-9]|webterm' )
  do
    case $i in
      *kvmd-3.29[2-9]*|*kvmd-3.[3-9]*|*kvmd-[45].[1-9]*)  # if latest/greatest is 3.292 and higher, then force 3.291 install
        echo "*** Force install kvmd 3.291 ***" | tee -a $LOGFILE
        i=$KVMDCACHE/$KVMDFILE
        ;;
      *)
        ;;
    esac

    echo "-> Extracting package $i into /" >> $INSTLOG
    tar xfJ $i
  done

  # uncompress janus package if /usr/bin/janus doesn't exist
  if [ ! -e /usr/bin/janus ]; then
    i=$( ls ${KVMDCACHE}/*.tar.xz | egrep janus | grep -v 1x )
    echo "-> Extracting package $i into /" >> $INSTLOG
    tar xfJ $i

  else      # confirm that /usr/bin/janus actually runs properly
    /usr/bin/janus --version > /dev/null 2>> $LOGFILE
    if [ $? -eq 0 ]; then
      echo "You have a working valid janus binary." | tee -a $LOGFILE
    else    # error status code, so uncompress from REPO package
      #i=$( ls ${KVMDCACHE}/*.tar.xz | egrep janus )
      #echo "-> Extracting package $i into /" >> $INSTLOG
      #tar xfJ $i
      apt-get remove janus janus-dev -y >> $LOGFILE
      apt-get install janus janus-dev -y >> $LOGFILE
    fi
  fi

  cd ${APP_PATH}
} # end install-kvmd-pkgs

fix-udevrules() {
  # for hdmiusb, replace %b with 1-1.4:1.0 in /etc/udev/rules.d/99-kvmd.rules
  sed -i -e 's+\%b+1-1.4:1.0+g' /etc/udev/rules.d/99-kvmd.rules | tee -a $LOGFILE
  echo
  cat /etc/udev/rules.d/99-kvmd.rules | tee -a $LOGFILE
} # end fix-udevrules

enable-kvmd-svcs() {
  # enable KVMD services but don't start them
  echo "-> Enabling $SERVICES services, but do not start them." | tee -a $LOGFILE
  systemctl enable $SERVICES
} # end enable-kvmd-svcs

build-ustreamer() {
  printf "\n\n-> Building ustreamer\n\n" | tee -a $LOGFILE
  # Install packages needed for building ustreamer source
  echo "apt install -y  libevent-dev libjpeg-dev libbsd-dev libgpiod-dev libsystemd-dev janus-dev janus" | tee -a $LOGFILE
  apt install -y  libevent-dev libjpeg-dev libbsd-dev libgpiod-dev libsystemd-dev janus-dev janus >> $LOGFILE

  # fix refcount.h
  sed -i -e 's|^#include "refcount.h"$|#include "../refcount.h"|g' /usr/include/janus/plugins/plugin.h

  # Download ustreamer source and build it
  cd /tmp
  unzip ${APP_PATH}/sources/ustreamer-6.12.zip
  cd ustreamer-6.12/
  #添加WITH_PYTHON=1 ，使kvmd-vnc正常工作
  make WITH_GPIO=1 WITH_SYSTEMD=1 WITH_JANUS=1 WITH_PYTHON=1 -j
  #删除 --prefix=$(PREFIX) ，修复无法安装pythgon包的问题
  sed -i 's/--prefix=\$(PREFIX)//g' python/Makefile
  make install WITH_PYTHON=1
  # kvmd service is looking for /usr/bin/ustreamer
  ln -sf /usr/local/bin/ustreamer* /usr/bin/

  # add janus support
  mkdir -p /usr/lib/ustreamer/janus
  cp /tmp/ustreamer-6.12/janus/libjanus_ustreamer.so /usr/lib/ustreamer/janus
} # end build-ustreamer

install-dependencies() {
  echo
  echo "-> Installing dependencies for pikvm" | tee -a $LOGFILE

  echo "apt install -y make nginx python3 gcc unzip net-tools bc expect v4l-utils iptables vim dos2unix screen tmate nfs-common gpiod ffmpeg dialog iptables dnsmasq git python3-pip tesseract-ocr tesseract-ocr-eng libasound2-dev libsndfile-dev libspeexdsp-dev  build-essential libssl-dev libffi-dev" | tee -a $LOGFILE
  apt install -y make nginx python3 gcc unzip net-tools bc expect v4l-utils iptables vim dos2unix screen tmate nfs-common gpiod ffmpeg dialog iptables dnsmasq git python3-pip tesseract-ocr tesseract-ocr-eng libasound2-dev libsndfile-dev libspeexdsp-dev build-essential libssl-dev libffi-dev >> $LOGFILE

  sed -i -e 's/#port=5353/port=5353/g' /etc/dnsmasq.conf

  install-python-packages

  

  echo "-> Make tesseract data link" | tee -a $LOGFILE
  ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata

  echo "-> Install TTYD" | tee -a $LOGFILE
  apt install -y ttyd | tee -a $LOGFILE
  /usr/bin/ttyd -v | tee -a $LOGFILE

  if [ ! -e /usr/local/bin/gpio ]; then
    printf "\n\n-> Building wiringpi from source\n\n" | tee -a $LOGFILE
    cd /tmp; rm -rf WiringPi-3.6
    unzip ${APP_PATH}/sources/WiringPi-3.6.zip
    cd WiringPi-3.6
    ./build
  else
    printf "\n\n-> Wiringpi (gpio) is already installed.\n\n" | tee -a $LOGFILE
  fi
  gpio -v | tee -a $LOGFILE

  echo "-> Install ustreamer" | tee -a $LOGFILE
  if [ ! -e /usr/bin/ustreamer ]; then
    cd /tmp
    ### required dependent packages for ustreamer ###
    build-ustreamer
    cd ${APP_PATH}
  fi
  echo -n "ustreamer version: " | tee -a $LOGFILE
  ustreamer -v | tee -a $LOGFILE
  ustreamer --features | tee -a $LOGFILE
} # end install-dependencies

python-pkg-dir() {
  # debian system python3 no alias
  # create quick python script to show where python packages need to go
  cat << MYSCRIPT > /tmp/syspath.py
#!$(which python3)
import sys
print (sys.path)
MYSCRIPT

  chmod +x /tmp/syspath.py

  #PYTHONDIR=$( /tmp/syspath.py | awk -F, '{print $NF}' | cut -d"'" -f2 )
  ### hardcode path for armbian/raspbian
  PYTHONDIR="/usr/lib/python3/dist-packages"
} # end python-pkg-dir

fix-nginx-symlinks() {
  # disable default nginx service since we will use kvmd-nginx instead
  echo
  echo "-> Disabling nginx service, so that we can use kvmd-nginx instead" | tee -a $LOGFILE
  systemctl disable --now nginx

  # setup symlinks
  echo
  echo "-> Creating symlinks for use with kvmd python scripts" | tee -a $LOGFILE
  if [ ! -e /usr/bin/nginx ]; then ln -sf /usr/sbin/nginx /usr/bin/; fi
  if [ ! -e /usr/sbin/python ]; then ln -sf /usr/bin/python3 /usr/sbin/python; fi
  if [ ! -e /usr/bin/iptables ]; then ln -sf /usr/sbin/iptables /usr/bin/iptables; fi
  if [ ! -e /usr/bin/vcgencmd ]; then ln -sf /opt/vc/bin/* /usr/bin/; chmod +x /opt/vc/bin/*; fi

  python-pkg-dir

  if [ ! -e $PYTHONDIR/kvmd ]; then
    # Debian python版本比 pikvm官方的低一些
    # in case new kvmd packages are now using python 3.11
    ln -sf /usr/lib/python3.1*/site-packages/kvmd* ${PYTHONDIR}
  fi
} # end fix-nginx-symlinks

fix-python-symlinks(){
  python-pkg-dir

  if [ ! -e $PYTHONDIR/kvmd ]; then
    # Debian python版本比 pikvm官方的低一些
    ln -sf /usr/lib/python3.1*/site-packages/kvmd* ${PYTHONDIR}
  fi
}

apply-custom-patch(){
  read -p "Do you want apply old kernel msd patch? [y/n]" answer
  case $answer in
    n|N|no|No)
      echo 'You skipped this patch.'
      ;;
    y|Y|Yes|yes)
      ./patches/custom/old-kernel-msd/apply.sh
      ;;
    *)
      echo "Try again.";;
  esac
}


fix-webterm() {
  echo
  echo "-> Creating kvmd-webterm homedir" | tee -a $LOGFILE
  mkdir -p /home/kvmd-webterm
  chown kvmd-webterm /home/kvmd-webterm
  ls -ld /home/kvmd-webterm | tee -a $LOGFILE

  # remove -W option since ttyd installed on raspbian/armbian is 1.6.3 (-W option only works with ttyd 1.7.x)
  _ttydver=$( /usr/bin/ttyd -v | awk '{print $NF}' )
  case $_ttydver in
    1.6*)
      echo "ttyd $_ttydver found.  Removing -W from /lib/systemd/system/kvmd-webterm.service"
      sed -i -e '/-W \\/d' /lib/systemd/system/kvmd-webterm.service
      ;;
    1.7*)
      echo "ttyd $_ttydver found.  Nothing to do."
      ;;
  esac

  # add sudoers entry for kvmd-webterm user to be able to run sudo
  echo "kvmd-webterm ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/kvmd-webterm; chmod 440 /etc/sudoers.d/kvmd-webterm
} # end fix-webterm

create-kvmdfix() {
  # Create kvmd-fix service and script
  cat <<ENDSERVICE > /lib/systemd/system/kvmd-fix.service
[Unit]
Description=KVMD Fixes
After=network.target network-online.target nss-lookup.target
Before=kvmd.service

[Service]
User=root
Type=simple
ExecStart=/usr/bin/kvmd-fix

[Install]
WantedBy=multi-user.target
ENDSERVICE

  cat <<SCRIPTEND > /usr/bin/kvmd-fix
#!/bin/bash
# Written by @srepac
# 1.  Properly set group ownership of /dev/gpio*
# 2.  fix /dev/kvmd-video symlink to point to /dev/video1 (Amglogic Device video0 is not usb device)
#
### These fixes are required in order for kvmd service to start properly
#
set -x
chgrp gpio /dev/gpio*
chmod 660 /dev/gpio*
ls -l /dev/gpio*

udevadm trigger
ls -l /dev/kvmd-video

if [ \$( systemctl | grep kvmd-oled | grep -c activ ) -eq 0 ]; then
  echo "kvmd-oled service is not enabled."
  exit 0
else
  echo "kvmd-oled service is enabled and activated."
fi

### kvmd-oled fix: swap i2c-0 <-> i2c-1  (code is looking for I2C oled on i2c-1)
# pins #1 - 3.3v, #3 - SDA, #5 - SCL, and #9 - GND
i2cget -y 0 0x3c
if [ \$? -eq 0 ]; then
  echo "-> Found valid I2C OLED at i2c-0.  Applying I2C OLED fix."
  cd /dev

  # rename i2c-0 -> i2c-9, move i2c-1 to i2c-0, and rename the good i2c-9 to i2c-1
  mv i2c-0 i2c-9
  mv i2c-1 i2c-0
  mv i2c-9 i2c-1

  # restart kvmd-oled service
  systemctl restart kvmd-oled
else
  echo "-> I2C OLED fix already applied and OLED should be showing info."
fi
SCRIPTEND

  chmod +x /usr/bin/kvmd-fix
} # end create-kvmdfix

set-ownership() {
  # set proper ownership of password files and kvmd-webterm homedir
  cd /etc/kvmd
  chown kvmd:kvmd htpasswd
  chown kvmd-ipmi:kvmd-ipmi ipmipasswd
  chown kvmd-vnc:kvmd-vnc vncpasswd
  chown kvmd-webterm /home/kvmd-webterm

  # add kvmd user to video group (this is required in order to use CSI bridge with OMX and h264 support)
  usermod -a -G video kvmd

  # add kvmd user to dialout group (required for xh_hk4401 kvm switch support)
  usermod -a -G dialout kvmd
} # end set-ownership

check-kvmd-works() {
  echo "-> Checking kvmd -m works before continuing" | tee -a $LOGFILE
  kvmd -m
  invalid=1
    ! $NOTCHROOT || while [ $invalid -eq 1 ]; do
    #kvmd -m
    read -p "Did kvmd -m run properly?  [y/n] " answer
    case $answer in
      n|N|no|No)
        echo "Please install missing packages as per the kvmd -m output in another ssh/terminal."
        ;;
      y|Y|Yes|yes)
        invalid=0
        ;;
      *)
        echo "Try again.";;
    esac
  done
} # end check-kvmd-works

start-kvmd-svcs() {
  #### start the main KVM services in order ####
  # 1. nginx is the webserver
  # 2. kvmd-otg is for OTG devices (keyboard/mouse, etc..)
  # 3. kvmd is the main daemon
  systemctl daemon-reload
  systemctl restart $SERVICES
} # end start-kvmd-svcs

fix-motd() {
  if [ -e /etc/motd ]; then rm /etc/motd; fi
  cp armbian/armbian-motd /usr/bin/
  chmod +x /usr/bin/armbian-motd
  chmod +x /etc/update-motd.d/*
  sed -i 's/cat \/etc\/motd/armbian-motd/g' /lib/systemd/system/kvmd-webterm.service
  systemctl daemon-reload
  # systemctl restart kvmd-webterm
} # end fix-motd

# 安装armbian的包
armbian-packages() {
  mkdir -p /opt/vc/bin/
  #cd /opt/vc/bin
  if [ ! -e /usr/bin/vcgencmd ]; then
    # Install vcgencmd for armbian platform
    cp -rf armbian/opt/* /opt/vc/bin
  else
    ln -s /usr/bin/vcgencmd /opt/vc/bin/
  fi
  #cp -rf armbian/udev /etc/

  cd ${APP_PATH}
} # end armbian-packages

fix-nfs-msd() {
  NAME="aiofiles.tar"

  LOCATION="/usr/lib/python3.11/site-packages"
  echo "-> Extracting $NAME into $LOCATION" | tee -a $LOGFILE
  tar xvf $NAME -C $LOCATION

  echo "-> Renaming original aiofiles and creating symlink to correct aiofiles" | tee -a $LOGFILE
  cd /usr/lib/python3/dist-packages
  mv aiofiles aiofiles.$(date +%Y%m%d.%H%M)
  ln -s $LOCATION/aiofiles .
  ls -ld aiofiles* | tail -5
}

fix-nginx() {
  #set -x
  KERNEL=$( uname -r | awk -F\- '{print $1}' )
  ARCH=$( uname -r | awk -F\- '{print $NF}' )
  echo "KERNEL:  $KERNEL   ARCH:  $ARCH" | tee -a $LOGFILE
  case $ARCH in
    ARCH) SEARCHKEY=nginx-mainline;;
    *) SEARCHKEY="nginx/";;
  esac

  HTTPSCONF="/etc/kvmd/nginx/listen-https.conf"
  echo "HTTPSCONF BEFORE:  $HTTPSCONF" | tee -a $LOGFILE
  cat $HTTPSCONF | tee -a $LOGFILE

  if [[ ! -e /usr/local/bin/pikvm-info || ! -e /tmp/pacmanquery ]]; then
    cp -f ${APP_PATH}/pikvm-info /usr/local/bin/pikvm-info
    chmod +x /usr/local/bin/pikvm-info
    echo "Getting list of packages installed..." | tee -a $LOGFILE
    pikvm-info > /dev/null    ### this generates /tmp/pacmanquery with list of installed pkgs
  fi

  NGINXVER=$( grep $SEARCHKEY /tmp/pacmanquery | awk '{print $1}' | cut -d'.' -f1,2 )
  echo
  echo "NGINX version installed:  $NGINXVER" | tee -a $LOGFILE

  case $NGINXVER in
    1.2[56789]|1.3*|1.4*|1.5*)   # nginx version 1.25 and higher
      cat << NEW_CONF > $HTTPSCONF
listen 443 ssl;
listen [::]:443 ssl;
http2 on;
NEW_CONF
      ;;

    1.18|*)   # nginx version 1.18 and lower
      cat << ORIG_CONF > $HTTPSCONF
listen 443 ssl http2;
listen [::]:443 ssl;
ORIG_CONF
      ;;

  esac

  echo "HTTPSCONF AFTER:  $HTTPSCONF" | tee -a $LOGFILE
  cat $HTTPSCONF | tee -a $LOGFILE
  set +x
} # end fix-nginx

ocr-fix() {  # create function
  echo
  echo "-> Apply OCR fix..." | tee -a $LOGFILE

  set -x
  # 1.  verify that Pillow module is currently running 9.0.x
  PILLOWVER=$( grep -i pillow $PIP3LIST | awk '{print $NF}' )

  case $PILLOWVER in
    9.*|8.*|7.*)   # Pillow running at 9.x and lower
      # 2.  update Pillow to 10.0.0
      pip3 install -U Pillow 2>> $LOGFILE

      # 3.  check that Pillow module is now running 10.0.0
      pip3 list | grep -i pillow | tee -a $LOGFILE

      #4.  restart kvmd and confirm OCR now works.
      systemctl restart kvmd
      ;;

    10.*|11.*|12.*)  # Pillow running at 10.x and higher
      echo "Already running Pillow $PILLOWVER.  Nothing to do." | tee -a $LOGFILE
      ;;

  esac

  set +x
  echo
} # end ocr-fix

async-lru-fix() {
  echo
  echo "-> Ensuring async-lru is installed with version 2.x ..." | tee -a $LOGFILE
  pip3 install async-lru 2> /dev/null
  PIP3LIST="/tmp/pip3.list"; /bin/rm -f $PIP3LIST
  pip3 list 2> /dev/null > $PIP3LIST

  ASYNCLRUVER=$( grep -i 'async[-_]lru' $PIP3LIST | awk '{print $NF}' )
  echo "ASYNC-LRU version:  $ASYNCLRUVER"
  case $ASYNCLRUVER in
    2.*) echo "Nothing to do.  aync-lru is already running $ASYNCLRUVER" | tee -a $LOFILE;;
    1.*|*) pip3 install -U async_lru --break-system-packages | tee -a $LOGFILE;;     # raspbian bookworm only installs 1.0.x, this forces 2.0.x
  esac
} # end async-lru-fix


#fix for onecloud
onecloud_conf(){
  if [ "$(hostname)" == "onecloud" ]; then
    pip3 config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple
    apt install -y cpufrequtils
    echo "-->Add /etc/rc.local and skip usbburning for onecloud"
    cat <<EOF >/etc/rc.local
#!/bin/bash
echo "default-on" >/sys/class/leds/onecloud\:green\:alive/trigger
echo "none" >/sys/class/leds/onecloud\:red\:alive/trigger
echo "none" >/sys/class/leds/onecloud\:blue\:alive/trigger
cpufreq-set -d 1200MHz -u 1200MHz
echo device > /sys/class/usb_role/c9040000.usb-role-switch/role
systemctl disable kvmd && systemctl stop kvmd
systemctl start kvmd
exit 0
EOF
    sed  -i '97c #' /usr/lib/python3.11/site-packages/kvmd/apps/kvmd/info/hw.py
    sed  -i '106c #' /usr/lib/python3.11/site-packages/kvmd/apps/kvmd/info/hw.py
    cp -f ./patches/onecloud_gpio.sh /usr/bin && chmod +x /usr/bin/onecloud_gpio.sh
    cp -f ${APP_PATH}/conf/override-onecloud.yaml /etc/kvmd/override.yaml
    #如果在CHROOT环境需设置NOTCHROOT=false
    ! $NOTCHROOT || gzip -dc ${APP_PATH}/patches/Boot_SkipUSBBurning.gz | dd of=/dev/mmcblk1 bs=512 seek=1 count=32767
    echo -e "\n"
    if [ ! -f `grep -c "onecloud_gpio.sh" /etc/sudoers`  ]; then
      echo kvmd ALL=\(ALL\) NOPASSWD: /usr/bin/onecloud_gpio.sh >>  /etc/sudoers
    fi
  fi
}


update-logo() {
  sed -i -e 's|class="svg-gray"|class="svg-color"|g' /usr/share/kvmd/web/index.html
  sed -i -e 's|target="_blank"><img class="svg-gray"|target="_blank"><img class="svg-color"|g' /usr/share/kvmd/web/kvm/index.html

  ### download opikvm-logo.svg and then overwrite logo.svg
  cp -f ${APP_PATH}/opikvm-logo.svg /usr/share/kvmd/web/share/svg/opikvm-logo.svg
  cd /usr/share/kvmd/web/share/svg
  cp logo.svg logo.svg.old
  cp opikvm-logo.svg logo.svg

  # change some text in the main html page
  #sed -i.bak -e 's/The Open Source KVM over IP/KVM over IP on non-Arch linux OS by @srepac/g' /usr/share/kvmd/web/index.html
  #sed -i.bak -e 's/The Open Source KVM over IP/KVM over IP on non-Arch linux OS by @srepac/g' /usr/share/kvmd/web/kvm/index.html
  #sed -i.backup -e 's|https://pikvm.org/support|https://discord.gg/YaJ87sVznc|g' /usr/share/kvmd/web/kvm/index.html
  #sed -i.backup -e 's|https://pikvm.org/support|https://discord.gg/YaJ87sVznc|g' /usr/share/kvmd/web/index.html
  cd
}


### MAIN STARTS HERE ###
# Install is done in two parts
# First part requires a reboot in order to create kvmd users and groups
# Second part will start the necessary kvmd services

# if /etc/kvmd/htpasswd exists, then make a backup
if [ -e /etc/kvmd/htpasswd ]; then cp /etc/kvmd/htpasswd /etc/kvmd/htpasswd.save; fi

### I uploaded all these into github on 05/22/23 -- so just copy them into correct location
cd ${APP_PATH}
cp -rf pistat /usr/local/bin/pistat
cp -rf pi-temp /usr/local/bin/pi-temp
cp -rf pikvm-info /usr/local/bin/pikvm-info
chmod +x /usr/local/bin/pi*

### fix for kvmd 3.230 and higher
ln -sf python3 /usr/bin/python

SERVICES="kvmd-nginx kvmd-webterm kvmd-otg kvmd kvmd-fix kvmd-vnc kvmd-ipmi"

# added option to re-install by adding -f parameter (for use as platform switcher)
PYTHON_VERSION=$( python3 -V | awk '{print $2}' | cut -d'.' -f1,2 )
if [[ $( grep kvmd /etc/passwd | wc -l ) -eq 0 || "$1" == "-f" ]]; then
  printf "\nRunning part 1 of PiKVM installer script v$VER by @srepac and @SilentWind\n" | tee -a $LOGFILE
  get-platform
  get-packages
  install-kvmd-pkgs
  boot-files
  create-override
  gen-ssl-certs
  fix-udevrules
  install-dependencies
  ! $NOTCHROOT ||  otg-devices
  armbian-packages
  onecloud_conf #set for onecloud
  systemctl disable --now janus ttyd

  printf "\nEnd part 1 of PiKVM installer script v$VER by @srepac and @SilentWind\n" >> $LOGFILE
  printf "\nReboot is required to create kvmd users and groups.\nPlease re-run this script after reboot to complete the install.\n" | tee -a $LOGFILE

  # Fix paste-as-keys if running python 3.7
  if [[ $( python3 -V | awk '{print $2}' | cut -d'.' -f1,2 ) == "3.7" ]]; then
    sed -i -e 's/reversed//g' /usr/lib/python3.1*/site-packages/kvmd/keyboard/printer.py
  fi

  ### run these to make sure kvmd users are created ###
  echo "-> Ensuring KVMD users and groups ..." | tee -a $LOGFILE
  systemd-sysusers /usr/lib/sysusers.d/kvmd.conf
  systemd-sysusers /usr/lib/sysusers.d/kvmd-webterm.conf

  # Ask user to press CTRL+C before reboot or ENTER to proceed with reboot
  echo
  ! $NOTCHROOT ||  read -p "Press ENTER to continue or CTRL+C to break out of script."
  ! $NOTCHROOT ||  reboot
else
  printf "\nRunning part 2 of PiKVM installer script v$VER by @srepac and @SilentWind\n" | tee -a $LOGFILE

  echo "-> Re-installing janus ..." | tee -a $LOGFILE
  apt reinstall -y janus > /dev/null 2>&1
  ### run these to make sure kvmd users are created ###
  echo "-> Ensuring KVMD users and groups ..." | tee -a $LOGFILE
  systemd-sysusers /usr/lib/sysusers.d/kvmd.conf
  systemd-sysusers /usr/lib/sysusers.d/kvmd-webterm.conf

  fix-nginx-symlinks
  fix-python-symlinks
  fix-webterm
  fix-motd
  fix-nfs-msd
  fix-nginx
  async-lru-fix
  ocr-fix
  
  set-ownership
  create-kvmdfix

  echo "-> Install python3 modules dbus_next and zstandard" | tee -a $LOGFILE
  if [[ "$PYTHONVER" == "3.11" ]]; then
    apt install -y python3-dbus-next python3-zstandard
  else
    pip3 install dbus_next zstandard
  fi
  ### additional python pip dependencies for kvmd 3.238 and higher
  case $PYTHONVER in
    3.10*|3.[987]*)
      pip3 install async-lru 2> /dev/null
      ### Fix for kvmd 3.291 -- only applies to python 3.10 ###
      sed -i -e 's|gpiod.EdgeEvent|gpiod.LineEvent|g' /usr/lib/python3/dist-packages/kvmd/aiogp.py
      sed -i -e 's|gpiod.line,|gpiod.Line,|g'         /usr/lib/python3/dist-packages/kvmd/aiogp.py
      ;;
    3.11*)
      pip3 install async-lru --break-system-packages 2> /dev/null
      ;;
  esac

  check-kvmd-works
  enable-kvmd-svcs
  update-logo
  start-kvmd-svcs

  printf "\nCheck kvmd devices\n\n" | tee -a $LOGFILE
  ls -l /dev/kvmd* | tee -a $LOGFILE
  printf "\nYou should see devices for keyboard, mouse, and video.\n" | tee -a $LOGFILE

  printf "\nPoint a browser to https://$(hostname)\nIf it doesn't work, then reboot one last time.\nPlease make sure kvmd services are running after reboot.\n" | tee -a $LOGFILE
fi

cd $CWD
cp -rf web.css /etc/kvmd/web.css

systemctl status $SERVICES | grep Loaded | tee -a $LOGFILE

### fix totp.secret file permissions for use with 2FA
chmod go+r /etc/kvmd/totp.secret
chown kvmd:kvmd /etc/kvmd/totp.secret

### create rw and ro so that /usr/bin/kvmd-bootconfig doesn't fail
touch /usr/local/bin/rw /usr/local/bin/ro
chmod +x /usr/local/bin/rw /usr/local/bin/ro

### update default hostname info in webui to reflect current hostname
sed -i -e "s/localhost.localdomain/`hostname`/g" /etc/kvmd/meta.yaml

### restore htpasswd from previous install, if applies
if [ -e /etc/kvmd/htpasswd.save ]; then cp /etc/kvmd/htpasswd.save /etc/kvmd/htpasswd; fi

### instead of showing # fps dynamic, show REDACTED fps dynamic instead;  USELESS fps meter fix
#sed -i -e 's|${__fps}|REDACTED|g' /usr/share/kvmd/web/share/js/kvm/stream_mjpeg.js

### fix kvmd-webterm 0.49 change that changed ttyd to kvmd-ttyd which broke webterm
sed -i -e 's/kvmd-ttyd/ttyd/g' /lib/systemd/system/kvmd-webterm.service

# get rid of this line, otherwise kvmd-nginx won't start properly since the nginx version is not 1.25 and higher
if [ -e /etc/kvmd/nginx/nginx.conf.mako ]; then
  sed -i -e '/http2 on;/d' /etc/kvmd/nginx/nginx.conf.mako
fi

systemctl restart kvmd-nginx kvmd-webterm kvmd
