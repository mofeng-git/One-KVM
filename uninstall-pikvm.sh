#!/bin/bash
# Script written by @srepac as requested by @Mark Jim
# This script perform uninstall of pikvm from raspbian.  This performs the following:
#
# 1. Stop/disable kvmd services
# 2. Remove the main kvmd package files based on what was installed (see /var/cache/kvmd/installed_ver.txt file)
#
# CAVEATS:
# 1. Script does not remove /usr/bin/ttyd (webterm), /usr/bin/ustreamer, /usr/bin/ustreamer-dump
# 2. Script does not remove /usr/bin/janus (webrtc) and all its dependent files also
# 3. Script does not remove any directories; you may end up with empty directories
###
# CHANGELOG:
# 1.0	20220218	created script
# 1.1	20220220	confirm uninstall and add -f option to perform destructive commands
# 1.2   20220225	restore original /etc/motd
# 2.0   20220225	save custom configs for possible restores during re-install later
VER=2.0

save-configs() {  ### save config files inside /etc/kvmd in case user re-installs pikvm later
  if [[ $f_flag -eq 1 ]]; then
    printf "\n-> Saving config files\n"

    # Save passwd files used by PiKVM
    cp /etc/kvmd/htpasswd /etc/kvmd/htpasswd.save
    cp /etc/kvmd/ipmipasswd /etc/kvmd/ipmipasswd.save
    cp /etc/kvmd/vncpasswd /etc/kvmd/vncpasswd.save

    # Save webUI name and overrides
    cp /etc/kvmd/meta.yaml /etc/kvmd/meta.yaml.save
    cp /etc/kvmd/override.yaml /etc/kvmd/override.yaml.save
    cp /etc/kvmd/web.css /etc/kvmd/web.css.save

    # Save Janus configs
    #cp /etc/kvmd/janus/janus.cfg /etc/kvmd/janus/janus.cfg.save

    # Save sudoers.d/99_kvmd
    cp /etc/sudoers.d/99_kvmd /etc/sudoers.d/99_kvmd.save
    cp /etc/sudoers.d/custom_commands /etc/sudoers.d/custom_commands.save
  fi
}

stop-disable-kvmd() {
  #for i in $( systemctl | grep kvmd | grep -v var | awk '{print $1}')

  for i in $( systemctl | grep kvmd | grep -v var | awk '$1||$2 ~ /kvmd/ {print $2, $1}' | sed 's/loaded //g' | cut -d' ' -f1 ) 
  do
    echo "-> Stopping/disabling ${i} ..."
    if [[ $f_flag -eq 1 ]]; then systemctl disable --now $i; fi
  done
} # end stop-disable-kvmd

# Determine what kvmd version was installed last
remove-kvmd-package() {
  printf "\nProceeding to remove kvmd package files\n" | tee -a $LOGFILE
  #KVMDVER=$( egrep 'kvmd-[0-9]' $INSTLOG | awk '{print $4}' | cut -d'-' -f2 | tail -1 )
  KVMDVER=$( pikvm-info | grep kvmd-platform | awk '{print $1}' ) 
  KVMDPKG="kvmd-${KVMDVER}"

  echo "Uninstalling ${KVMDPKG} from this system."
  for file in $( tar tvfJ /var/cache/kvmd/${KVMDPKG}* | awk '{print $NF}' | grep -v '/$' )
  do
    echo "-> Deleting /$file ..."
    if [[ $f_flag -eq 1 ]]; then rm /$file; fi
  done
} # end remove-kvmd-package

restore-motd() {
  if [[ $f_flag -eq 1 ]]; then
    if [ -e /etc/motd.orig ]; then cp -f /etc/motd.orig /etc/motd; fi
  fi
  cat /etc/motd
} # end restore-motd

are-you-sure() {
  invalidinput=1
  while [ $invalidinput -eq 1 ]; do
    read -p "Uninstall PiKVM from this system.  Are you sure? [y/n] " SURE
    case $SURE in
      Y|y) invalidinput=0 ;;
      N|n) echo "Exiting."; exit 0 ;;
      *) echo "Invalid input. try again."; invalidinput=1 ;;
    esac
  done
} # end are-you-sure fn



### MAIN STARTS HERE ###
if [ -e /usr/local/bin/rw ]; then rw; fi
mkdir -p /var/cache/kvmd   # create directory in case it hasn't been created yet (e.g. installer hasn't been run)
export INSTLOG="/var/cache/kvmd/installed_ver.txt"
export LOGFILE="/var/cache/kvmd/uninstall.log"; rm -f $LOGFILE
if [ ! -e $INSTLOG ]; then 
  echo "Install log missing.  Nothing to do." | tee -a $LOGFILE
  exit 1
fi

if [[ "$1" == "-f" ]]; then 
  printf "\n*** Actually perform destructive commands option set.\n\n"
  f_flag=1 
else 
  printf "\n*** Only SHOWING what will be performed.  Re-run with -f to actually perform destructive commands.\n\n"
  f_flag=0
fi

are-you-sure
save-configs | tee -a $LOGFILE
stop-disable-kvmd | tee -a $LOGFILE
restore-motd | tee -a $LOGFILE
remove-kvmd-package | tee -a $LOGFILE
if [ -e /usr/local/bin/ro ]; then ro; fi
