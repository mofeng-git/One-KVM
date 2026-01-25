#!/bin/bash
# One-KVM initialization script
# Container entrypoint to start the one-kvm service

set -e

# Start one-kvm with default options.
# Additional options can be passed via environment variables.

# Data directory (prefer DATA_DIR, keep ONE_KVM_DATA_DIR for backward compatibility)
DATA_DIR="${DATA_DIR:-${ONE_KVM_DATA_DIR:-/etc/one-kvm}}"
ARGS=(-d "$DATA_DIR")

# Enable HTTPS if requested
if [ "${ENABLE_HTTPS:-false}" = "true" ]; then
    ARGS+=(--enable-https)
fi

# Custom bind address
if [ -n "$BIND_ADDRESS" ]; then
    ARGS+=(-a "$BIND_ADDRESS")
fi

# Custom port
if [ -n "$HTTP_PORT" ]; then
    ARGS+=(-p "$HTTP_PORT")
fi

# Custom HTTPS port
if [ -n "$HTTPS_PORT" ]; then
    ARGS+=(--https-port "$HTTPS_PORT")
fi

# Verbosity level
if [ -n "$VERBOSE" ]; then
    case "$VERBOSE" in
        1) ARGS+=(-v) ;;
        2) ARGS+=(-vv) ;;
        3) ARGS+=(-vvv) ;;
    esac
fi

echo "[INFO] Starting one-kvm..."
echo "[INFO] Arguments: ${ARGS[*]}"

# Execute one-kvm
exec /usr/bin/one-kvm "${ARGS[@]}"
