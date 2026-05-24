#!/usr/bin/env bash
# Build Android APKs using the Docker build image.
# Usage: ./build/build-android.sh [arm64|armv7|all|help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DOCKERFILE="$PROJECT_ROOT/build/cross/Dockerfile.android"
IMAGE_NAME="${ONE_KVM_ANDROID_DOCKER_IMAGE:-one-kvm-android-build:cn}"

fail() {
    echo "Error: $*" >&2
    exit 1
}

build_android() {
    local arch="$1"
    local docker_build_args=()
    local docker_mount_args=()
    local gradle_distribution_url="${ONE_KVM_GRADLE_DISTRIBUTION_URL:-}"
    local gradle_distribution_url_cn="${ONE_KVM_GRADLE_DISTRIBUTION_URL_CN:-https://mirrors.cloud.tencent.com/gradle/gradle-9.1.0-bin.zip}"
    local gradle_network_timeout="${ONE_KVM_GRADLE_NETWORK_TIMEOUT:-120000}"
    local gradle_cache="${ONE_KVM_ANDROID_GRADLE_CACHE_DIR:-one-kvm-android-gradle-cache}"
    local cargo_registry_cache="${ONE_KVM_ANDROID_CARGO_REGISTRY_CACHE_DIR:-one-kvm-android-cargo-registry}"
    local cargo_git_cache="${ONE_KVM_ANDROID_CARGO_GIT_CACHE_DIR:-one-kvm-android-cargo-git}"

    add_cache_mount() {
        local source="$1"
        local target="$2"

        if [[ "$source" == /* || "$source" == ./* || "$source" == ../* ]]; then
            mkdir -p "$source"
            source="$(cd "$source" && pwd)"
        fi

        docker_mount_args+=("-v" "$source:$target")
    }

    if [[ "${CHINAMIRRO:-}" == "1" ]]; then
        docker_build_args+=("--build-arg" "CHINAMIRRO=1")
        if [[ -z "$gradle_distribution_url" ]]; then
            gradle_distribution_url="$gradle_distribution_url_cn"
        fi
    fi

    if [[ "${ONE_KVM_ANDROID_SKIP_DOCKER_BUILD:-0}" == "1" ]]; then
        echo "=== Skipping Android image build: $IMAGE_NAME ==="
    else
        echo "=== Building Android image: $IMAGE_NAME ==="
        docker build \
            -f "$DOCKERFILE" \
            -t "$IMAGE_NAME" \
            "${docker_build_args[@]}" \
            "$PROJECT_ROOT/build/cross"
    fi

    add_cache_mount "$gradle_cache" "/root/.gradle"
    add_cache_mount "$cargo_registry_cache" "/root/.cargo/registry"
    add_cache_mount "$cargo_git_cache" "/root/.cargo/git"

    echo "=== Building Android APK: $arch ==="
    docker run --rm \
        -v "$PROJECT_ROOT:/workspace" \
        "${docker_mount_args[@]}" \
        -w /workspace \
        -e "CHINAMIRRO=${CHINAMIRRO:-0}" \
        -e "ONE_KVM_GRADLE_DISTRIBUTION_URL=$gradle_distribution_url" \
        -e "ONE_KVM_GRADLE_DISTRIBUTION_URL_CN=$gradle_distribution_url_cn" \
        -e "ONE_KVM_GRADLE_NETWORK_TIMEOUT=$gradle_network_timeout" \
        "$IMAGE_NAME" \
        "$arch"
}

[[ -f "$DOCKERFILE" ]] || fail "Android Dockerfile not found: $DOCKERFILE"
command -v docker >/dev/null 2>&1 || fail "docker is required"

case "${1:-all}" in
all)
    build_android all
    ;;
arm64)
    build_android arm64
    ;;
armv7)
    build_android armv7
    ;;
help | --help | -h)
    cat <<'EOF'
Usage: build/build-android.sh [arch|help]

Commands:
  all     (default) Build arm64 and armv7 APKs
  arm64   Build only arm64 APK
  armv7   Build only ARMv7 APK
  help    Show this help

Examples:
  build/build-android.sh
  build/build-android.sh arm64
  CHINAMIRRO=1 build/build-android.sh all
  CHINAMIRRO=1 ONE_KVM_GRADLE_DISTRIBUTION_URL=https://mirrors.aliyun.com/macports/distfiles/gradle/gradle-9.1.0-bin.zip build/build-android.sh all

Environment:
  ONE_KVM_ANDROID_GRADLE_CACHE_DIR          Host Gradle cache path or Docker volume name
  ONE_KVM_ANDROID_CARGO_REGISTRY_CACHE_DIR  Host Cargo registry cache path or Docker volume name
  ONE_KVM_ANDROID_CARGO_GIT_CACHE_DIR       Host Cargo git cache path or Docker volume name
  ONE_KVM_ANDROID_SKIP_DOCKER_BUILD=1       Reuse an already loaded Docker image

APK output:
  target/android/
EOF
    ;;
*)
    fail "Unknown argument: $1"
    ;;
esac
