#!/bin/bash
# One-KVM Docker Image Packaging Script
# Packages pre-compiled binaries into runtime Docker images
#
# Prerequisites:
#   1. Build binaries first: cross build --release --target <target>
#   2. Docker with buildx support
#
# Usage:
#   ./build-docker.sh --platform linux/amd64 --load
#   ./build-docker.sh --platform linux/arm64 --load
#   ./build-docker.sh --push --tag v1.0.0

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
echo_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
echo_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Configuration
REGISTRY="${REGISTRY:-}"  # e.g., docker.io/username or ghcr.io/username
IMAGE_NAME="${IMAGE_NAME:-one-kvm}"
TAG="${TAG:-latest}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STAGING_DIR="$PROJECT_ROOT/build-staging"

# Full image name with registry
get_full_image_name() {
    if [ -n "$REGISTRY" ]; then
        echo "$REGISTRY/$IMAGE_NAME"
    else
        echo "$IMAGE_NAME"
    fi
}

# Detect current platform
CURRENT_ARCH=$(uname -m)
case "$CURRENT_ARCH" in
    x86_64)      DEFAULT_PLATFORM="linux/amd64" ;;
    aarch64)     DEFAULT_PLATFORM="linux/arm64" ;;
    armv7l)      DEFAULT_PLATFORM="linux/arm/v7" ;;
    *)           DEFAULT_PLATFORM="linux/amd64" ;;
esac

# Parse arguments
PLATFORMS=""
PUSH=false
LOAD=false
BUILD_BINARY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --platform)
            PLATFORMS="$2"
            shift 2
            ;;
        --push)
            PUSH=true
            shift
            ;;
        --load)
            LOAD=true
            shift
            ;;
        --tag)
            TAG="$2"
            shift 2
            ;;
        --registry)
            REGISTRY="$2"
            shift 2
            ;;
        --build)
            BUILD_BINARY=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Package pre-compiled One-KVM binaries into Docker images."
            echo ""
            echo "Options:"
            echo "  --platform PLATFORM   Target platform (linux/amd64, linux/arm64, linux/arm/v7)"
            echo "                        Use comma to specify multiple: linux/amd64,linux/arm64"
            echo "                        Default: $DEFAULT_PLATFORM"
            echo "  --registry REGISTRY   Container registry (e.g., docker.io/user, ghcr.io/user)"
            echo "  --push                Push image to registry"
            echo "  --load                Load image to local Docker (single platform only)"
            echo "  --tag TAG             Image tag (default: latest)"
            echo "  --build               Also build the binary with cross (optional)"
            echo "  --help                Show this help"
            echo ""
            echo "Examples:"
            echo "  # Build for current platform and load locally"
            echo "  $0 --platform linux/arm64 --load"
            echo ""
            echo "  # Build and push single platform"
            echo "  $0 --platform linux/arm64 --registry docker.io/user --push"
            echo ""
            echo "  # Build multi-arch and push (creates unified manifest)"
            echo "  $0 --platform linux/amd64,linux/arm64,linux/arm/v7 --registry docker.io/user --push"
            exit 0
            ;;
        *)
            echo_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Default platform
if [ -z "$PLATFORMS" ]; then
    PLATFORMS="$DEFAULT_PLATFORM"
fi

# Validate single platform for --load
if [ "$LOAD" = true ] && [[ "$PLATFORMS" == *","* ]]; then
    echo_error "Cannot use --load with multiple platforms"
    exit 1
fi

# Map platform to Rust target
platform_to_target() {
    case "$1" in
        "linux/amd64")   echo "x86_64-unknown-linux-gnu" ;;
        "linux/arm64")   echo "aarch64-unknown-linux-gnu" ;;
        "linux/arm/v7")  echo "armv7-unknown-linux-gnueabihf" ;;
        *) echo_error "Unknown platform: $1"; exit 1 ;;
    esac
}

# Map platform to tool download names
get_tool_urls() {
    local platform="$1"
    case "$platform" in
        "linux/amd64")
            TTYD_URL="https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.x86_64"
            GOSTC_URL="https://github.com/SianHH/gostc-open/releases/download/v2.0.9/gostc_linux_amd64_v1.tar.gz"
            EASYTIER_URL="https://github.com/EasyTier/EasyTier/releases/download/v2.4.5/easytier-linux-x86_64-v2.4.5.zip"
            EASYTIER_DIR="easytier-linux-x86_64"
            ;;
        "linux/arm64")
            TTYD_URL="https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.aarch64"
            GOSTC_URL="https://github.com/SianHH/gostc-open/releases/download/v2.0.9/gostc_linux_arm64_v8.0.tar.gz"
            EASYTIER_URL="https://github.com/EasyTier/EasyTier/releases/download/v2.4.5/easytier-linux-aarch64-v2.4.5.zip"
            EASYTIER_DIR="easytier-linux-aarch64"
            ;;
        "linux/arm/v7")
            TTYD_URL="https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.armhf"
            GOSTC_URL="https://github.com/SianHH/gostc-open/releases/download/v2.0.9/gostc_linux_arm_7.tar.gz"
            EASYTIER_URL="https://github.com/EasyTier/EasyTier/releases/download/v2.4.5/easytier-linux-armv7hf-v2.4.5.zip"
            EASYTIER_DIR="easytier-linux-armv7hf"
            ;;
    esac
}

# Download tools for a platform
download_tools() {
    local platform="$1"
    local staging="$2"

    get_tool_urls "$platform"

    echo_info "Downloading tools for $platform..."

    # ttyd
    if [ ! -f "$staging/ttyd" ]; then
        curl -fsSL "$TTYD_URL" -o "$staging/ttyd"
        chmod +x "$staging/ttyd"
    fi

    # gostc
    if [ ! -f "$staging/gostc" ]; then
        curl -fsSL "$GOSTC_URL" -o /tmp/gostc.tar.gz
        tar -xzf /tmp/gostc.tar.gz -C "$staging"
        chmod +x "$staging/gostc"
        rm /tmp/gostc.tar.gz
    fi

    # easytier
    if [ ! -f "$staging/easytier-core" ]; then
        curl -fsSL "$EASYTIER_URL" -o /tmp/easytier.zip
        unzip -o /tmp/easytier.zip -d /tmp/easytier
        cp "/tmp/easytier/$EASYTIER_DIR/easytier-core" "$staging/easytier-core"
        chmod +x "$staging/easytier-core"
        rm -rf /tmp/easytier.zip /tmp/easytier
    fi
}

# Build and package for a single platform
build_for_platform() {
    local platform="$1"
    local target=$(platform_to_target "$platform")
    local staging="$STAGING_DIR/$target"

    echo_info "=========================================="
    echo_info "Processing: $platform ($target)"
    echo_info "=========================================="

    # Create staging directory
    mkdir -p "$staging/ventoy"

    # Build binary if requested
    if [ "$BUILD_BINARY" = true ]; then
        echo_info "Building binary with cross..."
        cd "$PROJECT_ROOT"
        cross build --release --target "$target"
    fi

    # Check binary exists
    local binary="$PROJECT_ROOT/target/$target/release/one-kvm"
    if [ ! -f "$binary" ]; then
        echo_error "Binary not found: $binary"
        echo_error "Build it first: cross build --release --target $target"
        exit 1
    fi

    # Copy binary to staging
    echo_info "Copying binary..."
    cp "$binary" "$staging/one-kvm"

    # Download tools
    download_tools "$platform" "$staging"

    # Copy init script
    cp "$PROJECT_ROOT/build/init.sh" "$staging/init.sh"

    # Copy ventoy resources (decompress xz files if needed)
    local ventoy_src="$PROJECT_ROOT/libs/ventoy-img-rs/resources"
    if [ -d "$ventoy_src" ]; then
        echo_info "Copying Ventoy resources..."
        # Copy boot.img directly
        if [ -f "$ventoy_src/boot.img" ]; then
            cp "$ventoy_src/boot.img" "$staging/ventoy/"
        fi
        # Decompress xz files
        if [ -f "$ventoy_src/core.img.xz" ]; then
            xz -dk "$ventoy_src/core.img.xz" -c > "$staging/ventoy/core.img"
        fi
        if [ -f "$ventoy_src/ventoy.disk.img.xz" ]; then
            xz -dk "$ventoy_src/ventoy.disk.img.xz" -c > "$staging/ventoy/ventoy.disk.img"
        fi
    else
        echo_warn "Ventoy resources not found at $ventoy_src"
    fi

    # Copy Dockerfile
    cp "$PROJECT_ROOT/build/Dockerfile.runtime" "$staging/Dockerfile"

    # Build Docker image
    echo_info "Building Docker image..."

    local full_image=$(get_full_image_name)
    local arch_tag="${target//_/-}"

    local build_cmd="docker buildx build --platform $platform"
    build_cmd="$build_cmd --build-arg TARGETPLATFORM=$platform"

    if [ "$PUSH" = true ]; then
        build_cmd="$build_cmd --push"
    elif [ "$LOAD" = true ]; then
        build_cmd="$build_cmd --load"
    fi

    # For multi-platform push, only tag with arch-specific name
    # The unified tag will be created via manifest later
    if [ "$PUSH" = true ] && [[ "$PLATFORMS" == *","* ]]; then
        build_cmd="$build_cmd -t $full_image:$TAG-$arch_tag"
    else
        build_cmd="$build_cmd -t $full_image:$TAG"
        build_cmd="$build_cmd -t $full_image:$TAG-$arch_tag"
    fi

    build_cmd="$build_cmd $staging"

    echo_info "Running: $build_cmd"
    eval "$build_cmd"

    echo_info "Done: $full_image:$TAG-$arch_tag"
}

# Main
main() {
    local full_image=$(get_full_image_name)

    echo_info "One-KVM Docker Image Builder"
    echo_info "Image: $full_image:$TAG"
    echo_info "Platforms: $PLATFORMS"
    if [ -n "$REGISTRY" ]; then
        echo_info "Registry: $REGISTRY"
    fi
    echo ""

    # Validate: push requires registry for multi-arch
    if [ "$PUSH" = true ] && [ -z "$REGISTRY" ]; then
        echo_warn "No registry specified. Images will be pushed to Docker Hub default."
        echo_warn "Consider using --registry to specify a registry."
    fi

    # Process each platform
    IFS=',' read -ra PLATFORM_ARRAY <<< "$PLATFORMS"

    for platform in "${PLATFORM_ARRAY[@]}"; do
        build_for_platform "$platform"
        echo ""
    done

    # Create multi-arch manifest if pushing multiple platforms
    if [ "$PUSH" = true ] && [ ${#PLATFORM_ARRAY[@]} -gt 1 ]; then
        echo_info "Creating multi-arch manifest..."

        local manifest_images=""
        for platform in "${PLATFORM_ARRAY[@]}"; do
            local target=$(platform_to_target "$platform")
            local arch_tag="${target//_/-}"
            manifest_images="$manifest_images $full_image:$TAG-$arch_tag"
        done

        # Remove existing manifest if any
        docker manifest rm "$full_image:$TAG" 2>/dev/null || true

        # Create and push manifest
        echo_info "Creating manifest: $full_image:$TAG"
        echo_info "Source images:$manifest_images"

        if docker manifest create "$full_image:$TAG" $manifest_images; then
            echo_info "Pushing manifest..."
            docker manifest push "$full_image:$TAG"
            echo_info "Multi-arch manifest pushed: $full_image:$TAG"
        else
            echo_warn "docker manifest failed, trying buildx imagetools..."
            docker buildx imagetools create -t "$full_image:$TAG" $manifest_images
        fi
    fi

    echo_info "=========================================="
    echo_info "Build completed successfully!"
    echo_info "=========================================="

    if [ "$LOAD" = true ]; then
        echo ""
        echo "Run the container:"
        echo "  docker run -d --privileged \\"
        echo "    -p 8080:8080 \\"
        echo "    -v /dev:/dev \\"
        echo "    $full_image:$TAG"
    fi

    if [ "$PUSH" = true ] && [ ${#PLATFORM_ARRAY[@]} -gt 1 ]; then
        echo ""
        echo "Multi-arch image available:"
        echo "  docker pull $full_image:$TAG"
        echo ""
        echo "Or pull specific architecture:"
        for platform in "${PLATFORM_ARRAY[@]}"; do
            local target=$(platform_to_target "$platform")
            local arch_tag="${target//_/-}"
            echo "  docker pull $full_image:$TAG-$arch_tag  # $platform"
        done
    fi
}

main "$@"
