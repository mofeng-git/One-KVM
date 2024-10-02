#!/bin/bash

echo $ATX
case $ATX in
    GPIO)
    CUSTOMATX=gpio
    ;;
    USBRELAY_HID)
    CUSTOMATX=usbrelay_hid
    ;;
    *)
    echo "No thing."
    exit -1
esac

#$1 option: short long reset
exec /etc/kvmd/custom_atx/$CUSTOMATX.sh $1