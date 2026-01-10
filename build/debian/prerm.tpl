#!/bin/bash
# Pre-removal script for one-kvm

set -e

case "$1" in
    remove|purge)
        # Stop service if running
        if [ -d /run/systemd/system ]; then
            systemctl stop one-kvm || true
            systemctl disable one-kvm || true
        fi
        ;;
    upgrade|deconfigure)
        # Keep data on upgrade
        :
        ;;
    failed-upgrade)
        # Handle upgrade failure
        :
        ;;
    *)
        echo "prerm called with unknown argument: $1" >&2
        exit 1
        ;;
esac

exit 0
