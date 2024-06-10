#!/bin/bash
case $1 in
    short)
    gpioset -m time -s 1 gpiochip1 7=0
    gpioset gpiochip1 7=1
    ;;
    long)
    gpioset -m time -s 5 gpiochip1 7=0
    gpioset gpiochip1 7=1
    ;;
    *)
    echo "No thing."
esac
