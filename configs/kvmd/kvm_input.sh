#!/bin/bash
# Written by @srepac   FILENAME:  input.sh
# Input switcher script for use with KVM switches that use CTRL+CTRL+#
# ... pass in # into the script
#
usage() {
  echo "usage: $0 <#> <pikvm-name-or-ip>        where # is the input number on the KVM switch"
  exit 1
}
password=admin

#HOTKEY="ScrollLock"
HOTKEY="ControlLeft"

if [[ "$1" == "" ]]; then
  usage
else
  NUM="$1"
fi

if [[ "$2" == "" ]]; then
  IP="localhost"
else
  IP="$2"
fi

OSD=$( echo $HOTKEY | sed -e 's/ControlLeft/CTRL/g' )
echo "Sending $OSD + $OSD + $NUM to $IP"

curl -X POST -k -u admin:$password "https://$IP/api/hid/events/send_key?key=$HOTKEY" 2> /dev/null
curl -X POST -k -u admin:$password "https://$IP/api/hid/events/send_key?key=$HOTKEY" 2> /dev/null
curl -X POST -k -u admin:$password "https://$IP/api/hid/events/send_key?key=Digit${NUM}" 2> /dev/null