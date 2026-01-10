#!/bin/bash
# Post-installation script for one-kvm

set -e

case "$1" in
    configure|abort-upgrade|abort-remove|abort-deconfigure)
        # Create data directory
        mkdir -p /var/lib/one-kvm/ventoy
        mkdir -p /var/log/one-kvm

        # Set permissions
        chmod 755 /var/lib/one-kvm
        chmod 755 /var/lib/one-kvm/ventoy
        chmod 755 /var/log/one-kvm

        # Enable and start service (if systemd is available)
        if [ -d /run/systemd/system ]; then
            systemctl daemon-reload
            systemctl enable one-kvm
            # Don't start here, let user configure first
        fi
        ;;
    triggered)
        # Handle triggers (e.g., systemd restart)
        if [ -d /run/systemd/system ]; then
            systemctl restart one-kvm || true
        fi
        ;;
    abort-rollback|failed-upgrade)
        exit 0
        ;;
    *)
        echo "postinst called with unknown argument: $1" >&2
        exit 1
        ;;
esac

exit 0
