#!/bin/bash
# Build deb packages from pre-compiled binaries
# Binaries are compiled once on Debian 11 (GLIBC 2.31) via build-images.sh
# This script packages them directly on the host using dpkg-deb
# Usage: ./build/build-deb.sh [arch]
# Example: ./build/build-deb.sh aarch64

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Version from Cargo.toml
VERSION=$(grep -m1 '^version =' "$PROJECT_DIR/Cargo.toml" | cut -d'"' -f2)
if [ -z "$VERSION" ]; then
    echo "Error: Could not extract version from Cargo.toml"
    exit 1
fi

OUTPUT_DIR="$PROJECT_DIR/target/debian"
mkdir -p "$OUTPUT_DIR"

# Supported architectures
TARGETS=(
    "x86_64-unknown-linux-gnu:amd64"
    "aarch64-unknown-linux-gnu:arm64"
    "armv7-unknown-linux-gnueabihf:armhf"
)

# Package single architecture
package_arch() {
    local RUST_TARGET="$1"
    local DEB_ARCH="$2"

    echo "========================================"
    echo "Packaging: $RUST_TARGET -> $DEB_ARCH"
    echo "========================================"

    local BINARY_PATH="$PROJECT_DIR/target/$RUST_TARGET/release/one-kvm"
    if [[ ! -f "$BINARY_PATH" ]]; then
        echo "Error: Binary not found at $BINARY_PATH"
        echo "Please run ./build/build-images.sh first."
        return 1
    fi

    local PKG_DIR="/tmp/one-kvm-pkg-$$"
    local DEB_PATH="$OUTPUT_DIR/one-kvm_${VERSION}_${DEB_ARCH}.deb"

    # Create package structure
    mkdir -p "$PKG_DIR/DEBIAN"
    mkdir -p "$PKG_DIR/usr/bin"
    mkdir -p "$PKG_DIR/etc/one-kvm/ventoy"
    mkdir -p "$PKG_DIR/lib/systemd/system"

    # Copy binary
    cp "$BINARY_PATH" "$PKG_DIR/usr/bin/one-kvm"
    chmod 755 "$PKG_DIR/usr/bin/one-kvm"

    # Copy and process ventoy resources (decompress .xz files)
    if [ -d "$PROJECT_DIR/libs/ventoy-img-rs/resources" ]; then
        for file in "$PROJECT_DIR/libs/ventoy-img-rs/resources/"*; do
            if [ -f "$file" ]; then
                local filename=$(basename "$file")
                if [[ "$filename" == *.xz ]]; then
                    # Decompress xz files to target dir (not in-place)
                    xz -d -c "$file" > "$PKG_DIR/etc/one-kvm/ventoy/${filename%.xz}"
                else
                    cp "$file" "$PKG_DIR/etc/one-kvm/ventoy/"
                fi
            fi
        done
    fi

    # Copy systemd service file
    if [ -f "$SCRIPT_DIR/one-kvm.service" ]; then
        cp "$SCRIPT_DIR/one-kvm.service" "$PKG_DIR/lib/systemd/system/"
    fi

    # Create postinst script (enable service on install)
    cat > "$PKG_DIR/DEBIAN/postinst" <<'EOF'
#!/bin/bash
set -e

case "$1" in
    configure)
        # Enable and start service
        if [ -f /lib/systemd/system/one-kvm.service ]; then
            systemctl enable one-kvm
            systemctl start one-kvm || true
        fi
        ;;
    abort-upgrade|abort-deconfigure|abort-remove)
        ;;
    *)
        ;;
esac
exit 0
EOF
    chmod 755 "$PKG_DIR/DEBIAN/postinst"

    # Create prerm script (stop service on remove)
    cat > "$PKG_DIR/DEBIAN/prerm" <<'EOF'
#!/bin/bash
set -e

case "$1" in
    remove|deconfigure)
        if [ -f /lib/systemd/system/one-kvm.service ]; then
            systemctl stop one-kvm || true
            systemctl disable one-kvm || true
        fi
        ;;
    upgrade)
        if [ -f /lib/systemd/system/one-kvm.service ]; then
            systemctl stop one-kvm || true
        fi
        ;;
    failed-upgrade)
        ;;
    *)
        ;;
esac
exit 0
EOF
    chmod 755 "$PKG_DIR/DEBIAN/prerm"

    # Create control file
    BASE_DEPS="libc6 (>= 2.31), libgcc-s1, libstdc++6, libasound2 (>= 1.1), libdrm2 (>= 2.4)"
    AMD64_DEPS="libva2 (>= 2.0), libva-drm2 (>= 2.10), libva-x11-2 (>= 2.10), libmfx1 (>= 21.1), libx11-6 (>= 1.6), libxcb1 (>= 1.14)"
    DEPS="$BASE_DEPS"
    if [ "$DEB_ARCH" = "amd64" ]; then
        DEPS="$DEPS, $AMD64_DEPS"
    fi

    cat > "$PKG_DIR/DEBIAN/control" <<EOF
Package: one-kvm
Version: $VERSION
Section: admin
Priority: optional
Architecture: $DEB_ARCH
Depends: $DEPS
Maintainer: SilentWind <admin@mofeng.run>
Description: A open and lightweight IP-KVM solution
 Enables BIOS-level remote management of servers and workstations.
 Built on Debian 11, compatible with Debian 11+, Ubuntu 20.04+.
EOF

    # Build deb directly on host
    dpkg-deb --build "$PKG_DIR" "$DEB_PATH"

    rm -rf "$PKG_DIR"
    echo "Created: $DEB_PATH"
}

# Main
if [ -n "$1" ]; then
    # Package specific arch
    FOUND=0
    for target in "${TARGETS[@]}"; do
        IFS=':' read -r RUST_TARGET DEB_ARCH <<< "$target"
        if [[ "$1" == "$DEB_ARCH" ]] || [[ "$1" == "$RUST_TARGET" ]]; then
            package_arch "$RUST_TARGET" "$DEB_ARCH"
            FOUND=1
            break
        fi
    done

    if [ $FOUND -eq 0 ]; then
        echo "Error: Unknown architecture: $1"
        echo "Available: amd64, arm64, armhf"
        exit 1
    fi
else
    # Package all architectures
    for target in "${TARGETS[@]}"; do
        IFS=':' read -r RUST_TARGET DEB_ARCH <<< "$target"
        package_arch "$RUST_TARGET" "$DEB_ARCH"
    done
fi

echo ""
echo "========================================"
echo "All packages built successfully!"
echo "========================================"
ls -la "$OUTPUT_DIR"/*.deb
