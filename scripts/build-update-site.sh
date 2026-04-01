#!/usr/bin/env bash
#
# 生成 One-KVM 在线升级静态站点并打包为可部署 tar.gz。
# 输出目录结构：
#   <site_name>/v1/channels.json
#   <site_name>/v1/releases.json
#   <site_name>/v1/bin/<version>/one-kvm-<triple>
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSION=""
RELEASE_CHANNEL="stable"
STABLE_VERSION=""
BETA_VERSION=""
PUBLISHED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
ARTIFACTS_DIR=""
X86_64_BIN=""
AARCH64_BIN=""
ARMV7_BIN=""
X86_64_SET=0
AARCH64_SET=0
ARMV7_SET=0
SITE_NAME="one-kvm-update"
OUTPUT_FILE=""
OUTPUT_DIR="${PROJECT_ROOT}/dist"
declare -a NOTES=()

usage() {
    cat <<'EOF'
Usage:
  ./scripts/build-update-site.sh --version <x.x.x> [options]

Required:
  --version <x.x.x>                 Release 版本号（如 0.1.10）

Artifact input (二选一，可混用):
  --artifacts-dir <dir>             自动扫描目录中的标准文件名：
                                      one-kvm-x86_64-unknown-linux-gnu
                                      one-kvm-aarch64-unknown-linux-gnu
                                      one-kvm-armv7-unknown-linux-gnueabihf
  --x86_64 <file>                   指定 x86_64 二进制路径
  --aarch64 <file>                  指定 aarch64 二进制路径
  --armv7 <file>                    指定 armv7 二进制路径

Manifest options:
  --release-channel <stable|beta>   releases.json 里该版本所属渠道，默认 stable
  --stable <x.x.x>                  channels.json 的 stable 指针，默认等于 --version
  --beta <x.x.x>                    channels.json 的 beta 指针，默认等于 --version
  --published-at <RFC3339>          发布时间，默认当前 UTC 时间
  --note <text>                     发布说明，可重复传入多次

Output options:
  --site-name <name>                打包根目录名，默认 one-kvm-update
  --output-dir <dir>                输出目录（默认 <repo>/dist）
  --output <file.tar.gz>            输出包完整路径（优先级高于 --output-dir）

Other:
  -h, --help                        显示帮助

Example:
  ./scripts/build-update-site.sh \
    --version 0.1.10 \
    --artifacts-dir ./target/release \
    --release-channel stable \
    --stable 0.1.10 \
    --beta 0.1.11 \
    --note "修复 WebRTC 断流问题" \
    --note "优化 HID 输入延迟"
EOF
}

fail() {
    echo "Error: $*" >&2
    exit 1
}

require_cmd() {
    local cmd="$1"
    command -v "$cmd" >/dev/null 2>&1 || fail "Missing required command: ${cmd}"
}

json_escape() {
    local s="$1"
    s=${s//\\/\\\\}
    s=${s//\"/\\\"}
    s=${s//$'\n'/\\n}
    s=${s//$'\r'/\\r}
    s=${s//$'\t'/\\t}
    printf '%s' "$s"
}

is_valid_version() {
    local v="$1"
    [[ "$v" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

is_valid_channel() {
    local c="$1"
    [[ "$c" == "stable" || "$c" == "beta" ]]
}

while [[ $# -gt 0 ]]; do
    case "$1" in
    --version)
        VERSION="${2:-}"
        shift 2
        ;;
    --release-channel)
        RELEASE_CHANNEL="${2:-}"
        shift 2
        ;;
    --stable)
        STABLE_VERSION="${2:-}"
        shift 2
        ;;
    --beta)
        BETA_VERSION="${2:-}"
        shift 2
        ;;
    --published-at)
        PUBLISHED_AT="${2:-}"
        shift 2
        ;;
    --note)
        NOTES+=("${2:-}")
        shift 2
        ;;
    --artifacts-dir)
        ARTIFACTS_DIR="${2:-}"
        shift 2
        ;;
    --x86_64)
        X86_64_BIN="${2:-}"
        X86_64_SET=1
        shift 2
        ;;
    --aarch64)
        AARCH64_BIN="${2:-}"
        AARCH64_SET=1
        shift 2
        ;;
    --armv7)
        ARMV7_BIN="${2:-}"
        ARMV7_SET=1
        shift 2
        ;;
    --site-name)
        SITE_NAME="${2:-}"
        shift 2
        ;;
    --output-dir)
        OUTPUT_DIR="${2:-}"
        shift 2
        ;;
    --output)
        OUTPUT_FILE="${2:-}"
        shift 2
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        fail "Unknown argument: $1 (use --help)"
        ;;
    esac
done

require_cmd sha256sum
require_cmd stat
require_cmd tar
require_cmd mktemp

[[ -n "$VERSION" ]] || fail "--version is required"
is_valid_version "$VERSION" || fail "Invalid --version: ${VERSION} (expected x.x.x)"
is_valid_channel "$RELEASE_CHANNEL" || fail "Invalid --release-channel: ${RELEASE_CHANNEL}"

if [[ -z "$STABLE_VERSION" ]]; then
    STABLE_VERSION="$VERSION"
fi
if [[ -z "$BETA_VERSION" ]]; then
    BETA_VERSION="$VERSION"
fi
is_valid_version "$STABLE_VERSION" || fail "Invalid --stable: ${STABLE_VERSION}"
is_valid_version "$BETA_VERSION" || fail "Invalid --beta: ${BETA_VERSION}"

if [[ -n "$ARTIFACTS_DIR" ]]; then
    [[ -d "$ARTIFACTS_DIR" ]] || fail "--artifacts-dir not found: ${ARTIFACTS_DIR}"
    [[ -n "$X86_64_BIN" ]] || X86_64_BIN="${ARTIFACTS_DIR}/one-kvm-x86_64-unknown-linux-gnu"
    [[ -n "$AARCH64_BIN" ]] || AARCH64_BIN="${ARTIFACTS_DIR}/one-kvm-aarch64-unknown-linux-gnu"
    [[ -n "$ARMV7_BIN" ]] || ARMV7_BIN="${ARTIFACTS_DIR}/one-kvm-armv7-unknown-linux-gnueabihf"
fi

if [[ "$X86_64_SET" -eq 1 && ! -f "$X86_64_BIN" ]]; then
    fail "--x86_64 file not found: ${X86_64_BIN}"
fi
if [[ "$AARCH64_SET" -eq 1 && ! -f "$AARCH64_BIN" ]]; then
    fail "--aarch64 file not found: ${AARCH64_BIN}"
fi
if [[ "$ARMV7_SET" -eq 1 && ! -f "$ARMV7_BIN" ]]; then
    fail "--armv7 file not found: ${ARMV7_BIN}"
fi

declare -A SRC_BY_TRIPLE=()
if [[ -n "$X86_64_BIN" && -f "$X86_64_BIN" ]]; then
    SRC_BY_TRIPLE["x86_64-unknown-linux-gnu"]="$X86_64_BIN"
fi
if [[ -n "$AARCH64_BIN" && -f "$AARCH64_BIN" ]]; then
    SRC_BY_TRIPLE["aarch64-unknown-linux-gnu"]="$AARCH64_BIN"
fi
if [[ -n "$ARMV7_BIN" && -f "$ARMV7_BIN" ]]; then
    SRC_BY_TRIPLE["armv7-unknown-linux-gnueabihf"]="$ARMV7_BIN"
fi

if [[ ${#SRC_BY_TRIPLE[@]} -eq 0 ]]; then
    fail "No artifact found. Provide --artifacts-dir or at least one of --x86_64/--aarch64/--armv7."
fi

BUILD_DIR="$(mktemp -d)"
trap 'rm -rf "$BUILD_DIR"' EXIT

SITE_DIR="${BUILD_DIR}/${SITE_NAME}"
V1_DIR="${SITE_DIR}/v1"
BIN_DIR="${V1_DIR}/bin/${VERSION}"
mkdir -p "$BIN_DIR"

declare -A SHA_BY_TRIPLE=()
declare -A SIZE_BY_TRIPLE=()

TRIPLES=(
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    "armv7-unknown-linux-gnueabihf"
)

for triple in "${TRIPLES[@]}"; do
    src="${SRC_BY_TRIPLE[$triple]:-}"
    if [[ -z "$src" ]]; then
        continue
    fi
    [[ -f "$src" ]] || fail "Artifact not found for ${triple}: ${src}"

    dest_name="one-kvm-${triple}"
    dest_path="${BIN_DIR}/${dest_name}"
    cp "$src" "$dest_path"

    sha="$(sha256sum "$dest_path" | awk '{print $1}')"
    size="$(stat -c%s "$dest_path")"
    SHA_BY_TRIPLE["$triple"]="$sha"
    SIZE_BY_TRIPLE["$triple"]="$size"
done

cat >"${V1_DIR}/channels.json" <<EOF
{
  "stable": "${STABLE_VERSION}",
  "beta": "${BETA_VERSION}"
}
EOF

RELEASES_FILE="${V1_DIR}/releases.json"
{
    echo '{'
    echo '  "releases": ['
    echo '    {'
    echo "      \"version\": \"${VERSION}\","
    echo "      \"channel\": \"${RELEASE_CHANNEL}\","
    echo "      \"published_at\": \"${PUBLISHED_AT}\","

    if [[ ${#NOTES[@]} -eq 0 ]]; then
        echo '      "notes": [],'
    else
        echo '      "notes": ['
        for i in "${!NOTES[@]}"; do
            esc_note="$(json_escape "${NOTES[$i]}")"
            if [[ "$i" -lt $((${#NOTES[@]} - 1)) ]]; then
                echo "        \"${esc_note}\","
            else
                echo "        \"${esc_note}\""
            fi
        done
        echo '      ],'
    fi

    echo '      "artifacts": {'
    written=0
    for triple in "${TRIPLES[@]}"; do
        if [[ -z "${SHA_BY_TRIPLE[$triple]:-}" ]]; then
            continue
        fi
        url="/v1/bin/${VERSION}/one-kvm-${triple}"
        if [[ $written -eq 1 ]]; then
            echo ','
        fi
        cat <<EOF
        "${triple}": {
          "url": "${url}",
          "sha256": "${SHA_BY_TRIPLE[$triple]}",
          "size": ${SIZE_BY_TRIPLE[$triple]}
        }
EOF
        written=1
    done
    echo
    echo '      }'
    echo '    }'
    echo '  ]'
    echo '}'
} >"$RELEASES_FILE"

if [[ -n "$OUTPUT_FILE" ]]; then
    if [[ "$OUTPUT_FILE" != /* ]]; then
        OUTPUT_FILE="${PROJECT_ROOT}/${OUTPUT_FILE}"
    fi
else
    mkdir -p "$OUTPUT_DIR"
    OUTPUT_FILE="${OUTPUT_DIR}/${SITE_NAME}-${VERSION}.tar.gz"
fi

mkdir -p "$(dirname "$OUTPUT_FILE")"
tar -C "$BUILD_DIR" -czf "$OUTPUT_FILE" "$SITE_NAME"

echo "Build complete:"
echo "  package: ${OUTPUT_FILE}"
echo "  site root in tar: ${SITE_NAME}/"
echo "  release version: ${VERSION}"
echo "  release channel: ${RELEASE_CHANNEL}"
echo "  channels: stable=${STABLE_VERSION}, beta=${BETA_VERSION}"
echo "  artifacts:"
for triple in "${TRIPLES[@]}"; do
    if [[ -n "${SHA_BY_TRIPLE[$triple]:-}" ]]; then
        echo "    - ${triple}: size=${SIZE_BY_TRIPLE[$triple]} sha256=${SHA_BY_TRIPLE[$triple]}"
    fi
done
echo
echo "Deploy example:"
echo "  tar -xzf \"${OUTPUT_FILE}\" -C /var/www/"
echo "  # then ensure nginx root points to /var/www/${SITE_NAME}"
