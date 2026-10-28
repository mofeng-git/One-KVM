#!/usr/bin/env bash
#
# 生成 One-KVM 在线升级静态站点并打包为可部署 tar.gz。
# 输出目录结构：
#   <site_name>/v1/channels.json
#   <site_name>/v1/releases.json
#   <site_name>/v1/bin/<version>/one-kvm-<triple>
# 默认从 GitHub Release 下载 deb，提取二进制；也支持本地二进制输入。
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSION=""
RELEASE_CHANNEL="stable"
STABLE_VERSION=""
BETA_VERSION=""
PUBLISHED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
PUBLISHED_AT_SET=0
GITHUB_REPO="mofeng-git/One-KVM"
GITHUB_RELEASE=""
GITHUB_PROXY="${GITHUB_PROXY:-}"
GITHUB_API_PROXY="${GITHUB_API_PROXY:-}"
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
  ./scripts/build-update-site.sh [options]

GitHub input (无本地输入时默认使用 latest Release):
  --release <tag|url|latest>        GitHub Release 标签或完整页面 URL
  --github-repo <owner/repo>        默认 mofeng-git/One-KVM
  --github-proxy <url>              deb 下载代理前缀，如 https://ghproxy.com
                                   代理失败后自动尝试 GitHub 直连
  --github-api-proxy <url>          可选 GitHub API 代理前缀
                                   也可设置 GITHUB_PROXY / GITHUB_API_PROXY
  --version <x.x.x>                 GitHub 输入默认从 deb 文件名读取；本地输入必填
                                   GitHub 输入指定时校验版本一致

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
  --published-at <RFC3339>          发布时间，默认 GitHub 发布时间（本地输入为当前时间）
  --note <text>                     发布说明，可重复；指定后覆盖 GitHub 说明

Output options:
  --site-name <name>                打包根目录名，默认 one-kvm-update
  --output-dir <dir>                输出目录（默认 <repo>/dist）
  --output <file.tar.gz>            输出包完整路径（优先级高于 --output-dir）

Other:
  -h, --help                        显示帮助

Example:
  ./scripts/build-update-site.sh --release v261001

  ./scripts/build-update-site.sh \
    --release https://github.com/mofeng-git/One-KVM/releases/tag/v261001 \
    --github-proxy https://ghproxy.com

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
    if [[ "$1" != "-h" && "$1" != "--help" ]]; then
        [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || fail "Missing value for $1"
    fi
    case "$1" in
    --release | --github-tag)
        GITHUB_RELEASE="$2"
        shift 2
        ;;
    --github-repo)
        GITHUB_REPO="$2"
        shift 2
        ;;
    --github-proxy)
        GITHUB_PROXY="$2"
        shift 2
        ;;
    --github-api-proxy)
        GITHUB_API_PROXY="$2"
        shift 2
        ;;
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
        PUBLISHED_AT_SET=1
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

[[ "$SITE_NAME" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] || fail "Invalid --site-name: ${SITE_NAME}"
BUILD_DIR="$(mktemp -d)"
trap 'rm -rf "$BUILD_DIR"' EXIT

download_github() {
    local url="$1" dest="$2" proxy="$3"
    local -a candidates=()
    [[ -z "$proxy" ]] || candidates+=("${proxy%/}/${url}")
    candidates+=("$url")
    local candidate
    for candidate in "${candidates[@]}"; do
        echo "Downloading: $candidate" >&2
        if curl --fail --location --show-error --connect-timeout 15 --max-time 300 \
            --retry 2 --retry-delay 2 --output "${dest}.part" "$candidate"; then
            # 一些代理返回 HTTP 200 的 HTML 页面，不能当作 deb 或 API 响应。
            if [[ "$dest" == *.deb ]]; then
                dpkg-deb --info "${dest}.part" >/dev/null 2>&1 || continue
            else
                python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); assert isinstance(r,dict) and isinstance(r.get("assets"),list)' \
                    "${dest}.part" >/dev/null 2>&1 || continue
            fi
            mv "${dest}.part" "$dest"
            return 0
        fi
        echo "Download failed or invalid response; trying next source." >&2
    done
    fail "Unable to download $url"
}

if [[ -n "$GITHUB_RELEASE" || ( -z "$ARTIFACTS_DIR" && "$X86_64_SET" -eq 0 && "$AARCH64_SET" -eq 0 && "$ARMV7_SET" -eq 0 ) ]]; then
    [[ -z "$ARTIFACTS_DIR" && "$X86_64_SET" -eq 0 && "$AARCH64_SET" -eq 0 && "$ARMV7_SET" -eq 0 ]] || \
        fail "--release cannot be combined with local artifact input"
    require_cmd curl
    require_cmd python3
    require_cmd dpkg-deb
    GITHUB_RELEASE="${GITHUB_RELEASE:-latest}"
    if [[ "$GITHUB_RELEASE" =~ ^https://github\.com/([^/]+/[^/]+)/releases/tag/([^/?#]+)/?$ ]]; then
        GITHUB_REPO="${BASH_REMATCH[1]}"
        GITHUB_RELEASE="${BASH_REMATCH[2]}"
    fi
    [[ "$GITHUB_REPO" =~ ^[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+$ ]] || fail "Invalid --github-repo"
    for proxy in "$GITHUB_PROXY" "$GITHUB_API_PROXY"; do
        [[ -z "$proxy" || "$proxy" =~ ^https?://[^[:space:]]+$ ]] || fail "Invalid GitHub proxy URL: $proxy"
    done
    release_path="latest"
    if [[ "$GITHUB_RELEASE" != latest ]]; then
        release_tag="$(python3 -c 'import sys,urllib.parse; print(urllib.parse.quote(urllib.parse.unquote(sys.argv[1]), safe=""))' "$GITHUB_RELEASE")"
        release_path="tags/${release_tag}"
    fi
    release_json="${BUILD_DIR}/release.json"
    download_github "https://api.github.com/repos/${GITHUB_REPO}/releases/${release_path}" "$release_json" "$GITHUB_API_PROXY"
    python3 - "$release_json" "${BUILD_DIR}/assets.tsv" "${BUILD_DIR}/metadata.json" "$VERSION" <<'PY'
import json
import re
import sys

release = json.load(open(sys.argv[1], encoding="utf-8"))
assets = []
seen = set()
for asset in release["assets"]:
    match = re.fullmatch(r"one-kvm_(\d+\.\d+\.\d+)_(amd64|arm64|armhf)\.deb", asset["name"])
    if not match:
        continue
    version, arch = match.groups()
    if arch in seen:
        sys.exit(f"Error: Duplicate deb architecture: {arch}")
    seen.add(arch)
    url = asset["browser_download_url"]
    if not url.startswith("https://github.com/") or any(c in url for c in "\t\r\n"):
        sys.exit("Error: Invalid GitHub asset URL")
    digest = asset.get("digest") or "-"
    if digest != "-" and not re.fullmatch(r"sha256:[a-fA-F0-9]{64}", digest):
        sys.exit("Error: Invalid GitHub asset digest")
    assets.append((arch, version, url, digest, str(asset["size"])))
if not assets:
    sys.exit("Error: Release has no supported One-KVM deb assets")
versions = {a[1] for a in assets}
if len(versions) != 1:
    sys.exit("Error: Release deb versions differ")
version = versions.pop()
if sys.argv[4] and sys.argv[4] != version:
    sys.exit(f"Error: --version {sys.argv[4]} differs from GitHub deb version {version}")
with open(sys.argv[2], "w", encoding="utf-8") as output:
    for asset in assets:
        output.write("\t".join(asset) + "\n")
with open(sys.argv[3], "w", encoding="utf-8") as output:
    json.dump(dict(version=version, published_at=release["published_at"],
                   notes=release.get("body") or ""), output, ensure_ascii=False)
PY
    VERSION="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "${BUILD_DIR}/metadata.json")"
    if [[ "$PUBLISHED_AT_SET" -eq 0 ]]; then
        PUBLISHED_AT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["published_at"])' "${BUILD_DIR}/metadata.json")"
    fi
    if [[ ${#NOTES[@]} -eq 0 ]]; then
        mapfile -t NOTES < <(python3 -c 'import json,re,sys; body=json.load(open(sys.argv[1]))["notes"]; print("\n".join(re.sub(r"^[-*+]\s+", "", line.strip()) for line in body.splitlines() if line.strip()), end="")' "${BUILD_DIR}/metadata.json")
    fi
    ARTIFACTS_DIR="${BUILD_DIR}/binaries"
    mkdir -p "$ARTIFACTS_DIR"
    while IFS=$'\t' read -r arch deb_version url digest expected_size; do
        case "$arch" in
        amd64) triple="x86_64-unknown-linux-gnu" ;;
        arm64) triple="aarch64-unknown-linux-gnu" ;;
        armhf) triple="armv7-unknown-linux-gnueabihf" ;;
        esac
        deb="${BUILD_DIR}/one-kvm_${deb_version}_${arch}.deb"
        download_github "$url" "$deb" "$GITHUB_PROXY"
        [[ "$(stat -c%s "$deb")" == "$expected_size" ]] || fail "deb size mismatch: $arch"
        if [[ "$digest" != - ]]; then
            actual_sha="$(sha256sum "$deb" | awk '{print $1}')"
            [[ "$actual_sha" == "${digest#sha256:}" ]] || fail "deb SHA256 mismatch: $arch"
        fi
        [[ "$(dpkg-deb -f "$deb" Package)" == one-kvm ]] || fail "Unexpected deb package: $arch"
        [[ "$(dpkg-deb -f "$deb" Version)" == "$VERSION" ]] || fail "deb version mismatch: $arch"
        [[ "$(dpkg-deb -f "$deb" Architecture)" == "$arch" ]] || fail "deb architecture mismatch: $arch"
        extract_dir="${BUILD_DIR}/extract-${arch}"
        dpkg-deb -x "$deb" "$extract_dir"
        binary="${extract_dir}/usr/bin/one-kvm"
        [[ -f "$binary" && ! -L "$binary" && -s "$binary" ]] || fail "deb missing /usr/bin/one-kvm: $arch"
        cp "$binary" "${ARTIFACTS_DIR}/one-kvm-${triple}"
    done < "${BUILD_DIR}/assets.tsv"
fi

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
    echo "      \"published_at\": \"$(json_escape "$PUBLISHED_AT")\","

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
