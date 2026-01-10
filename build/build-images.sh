#!/bin/bash
# Build cross-compiled binaries using cross with custom Dockerfiles
# Usage: ./build/build-images.sh [arch]
# Example: ./build/build-images.sh x86_64

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Supported architectures (Rust target)
ARCH_MAP=(
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    "armv7-unknown-linux-gnueabihf"
)

# Build for specific architecture using cross
build_arch() {
    local rust_target="$1"

    echo "=== Building: $rust_target (via cross with custom Dockerfile) ==="
    cross build --release --target "$rust_target"
}

# Main
case "${1:-all}" in
    all)
        for target in "${ARCH_MAP[@]}"; do
            build_arch "$target"
        done
        ;;
    x86_64|arm64|armv7)
        case "$1" in
            x86_64) build_arch "x86_64-unknown-linux-gnu" ;;
            arm64) build_arch "aarch64-unknown-linux-gnu" ;;
            armv7) build_arch "armv7-unknown-linux-gnueabihf" ;;
        esac
        ;;
    help|--help|-h)
        echo "Usage: $0 [arch|help]"
        echo ""
        echo "Commands:"
        echo "  all     (default) Build all architectures"
        echo "  x86_64  Build only x86_64"
        echo "  arm64   Build only arm64"
        echo "  armv7   Build only ARMv7"
        echo ""
        echo "Examples:"
        echo "  $0              # Build all"
        echo "  $0 x86_64       # Build x86_64 only"
        exit 0
        ;;
    *)
        echo "Error: Unknown argument: $1"
        exit 1
        ;;
esac

echo ""
echo "Binaries built:"
for target in "${ARCH_MAP[@]}"; do
    if [ -f "$PROJECT_DIR/target/$target/release/one-kvm" ]; then
        echo "  $target: OK"
    fi
done
echo ""
echo "Static libraries:"
for target in "${ARCH_MAP[@]}"; do
    case "$target" in
        x86_64-unknown-linux-gnu) gnu_target="x86_64-linux-gnu" ;;
        aarch64-unknown-linux-gnu) gnu_target="aarch64-linux-gnu" ;;
        armv7-unknown-linux-gnueabihf) gnu_target="armv7-linux-gnueabihf" ;;
    esac
    if [ -d "$PROJECT_DIR/target/one-kvm-libs/$gnu_target/lib" ]; then
        echo "  $gnu_target: OK"
    fi
done
echo ""
echo "Next step: ./build/package-docker.sh or ./build/package-deb.sh"
