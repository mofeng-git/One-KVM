#!/bin/bash

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

cleanup() {
    if [ "$OTG" == "1" ]; then
        echo "Trying exit OTG Port..." \
            && python -m kvmd.apps.otg stop \
            || echo -e "${RED}Failed to exit OTG Port${NC}"
        rm -r /run/kvmd/otg
    fi
    exit 0
}

trap cleanup SIGTERM

while true; do
    sleep 3600
done