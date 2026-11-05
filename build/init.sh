#!/bin/bash
# One-KVM initialization script
# Container entrypoint to start the one-kvm service

set -e

detect_intel_libva_driver() {
    if [ -n "${LIBVA_DRIVER_NAME:-}" ]; then
        echo "[INFO] Using preconfigured LIBVA_DRIVER_NAME=$LIBVA_DRIVER_NAME"
        return
    fi

    if [ "$(uname -m)" != "x86_64" ]; then
        return
    fi

    local devices=()
    if [ -n "${LIBVA_DEVICE:-}" ]; then
        devices=("$LIBVA_DEVICE")
    else
        shopt -s nullglob
        devices=(/dev/dri/renderD*)
        shopt -u nullglob
    fi

    if [ ${#devices[@]} -eq 0 ]; then
        return
    fi

    local device=""
    local node=""
    local vendor=""
    local driver=""

    for device in "${devices[@]}"; do
        if [ ! -e "$device" ]; then
            continue
        fi

        node="$(basename "$device")"
        vendor=""
        if [ -r "/sys/class/drm/$node/device/vendor" ]; then
            vendor="$(cat "/sys/class/drm/$node/device/vendor")"
        fi

        if [ -n "$vendor" ] && [ "$vendor" != "0x8086" ]; then
            echo "[INFO] Skipping VA-API probe for $device (vendor=$vendor)"
            continue
        fi

        for driver in iHD i965; do
            if LIBVA_DRIVER_NAME="$driver" vainfo --display drm --device "$device" >/dev/null 2>&1; then
                export LIBVA_DRIVER_NAME="$driver"
                if [ -n "$vendor" ]; then
                    echo "[INFO] Detected Intel VA-API driver '$driver' on $device (vendor=$vendor)"
                else
                    echo "[INFO] Detected Intel VA-API driver '$driver' on $device"
                fi
                return
            fi
        done
    done

    echo "[WARN] Unable to auto-detect an Intel VA-API driver; leaving LIBVA_DRIVER_NAME unset"
}

detect_intel_libva_driver

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
