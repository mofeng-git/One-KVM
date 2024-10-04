#!/bin/bash

echo $ATX
case $ATX in
    GPIO)
    sudo /etc/kvmd/custom_atx/gpio.sh $1
    ;;
    USBRELAY_HID)
    sudo /etc/kvmd/custom_atx/usbrelay_hid.sh $1
    ;;
    *)
    echo "No thing."
    exit -1
esac