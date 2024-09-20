#!/bin/bash

cleanup() {
    if [ "$OTG" == "1" ]; then
        echo "Trying exit OTG Port..."
        python -m kvmd.apps.otg stop
    fi
    exit 0
}

trap cleanup SIGTERM

while true; do
    sleep 3600
done