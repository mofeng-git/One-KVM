#!/bin/bash
case $1 in
    short)
    gpioset -m time -s 1 SHUTDOWNPIN=0
    gpioset SHUTDOWNPIN=1
    ;;
    long)
    gpioset -m time -s 5 SHUTDOWNPIN=0
    gpioset SHUTDOWNPIN=1
    ;;
    reset)
    gpioset -m time -s 1 REBOOTPIN=0
    gpioset REBOOTPIN=1
    ;;
    *)
    echo "No thing."
esac