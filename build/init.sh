#!/bin/bash
# One-KVM initialization script
# Container entrypoint to start the one-kvm service

set -e

# Start one-kvm with default options
# Additional options can be passed via environment variables
EXTRA_ARGS="-d /etc/one-kvm"

# Enable HTTPS if requested
if [ "${ENABLE_HTTPS:-false}" = "true" ]; then
    EXTRA_ARGS="$EXTRA_ARGS --enable-https"
fi

# Custom bind address
if [ -n "$BIND_ADDRESS" ]; then
    EXTRA_ARGS="$EXTRA_ARGS -a $BIND_ADDRESS"
fi

# Custom port
if [ -n "$HTTP_PORT" ]; then
    EXTRA_ARGS="$EXTRA_ARGS -p $HTTP_PORT"
fi

# Verbosity level
if [ -n "$VERBOSE" ]; then
    case "$VERBOSE" in
        1) EXTRA_ARGS="$EXTRA_ARGS -v" ;;
        2) EXTRA_ARGS="$EXTRA_ARGS -vv" ;;
        3) EXTRA_ARGS="$EXTRA_ARGS -vvv" ;;
    esac
fi

echo "[INFO] Starting one-kvm..."
echo "[INFO] Extra arguments: $EXTRA_ARGS"

# Execute one-kvm
exec /usr/bin/one-kvm $EXTRA_ARGS
