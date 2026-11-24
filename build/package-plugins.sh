#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

target="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
if (( $# > 0 )); then shift; fi
if (( $# == 0 )); then set -- rustdesk vnc rtsp easytier airferry easyfrp frpc gostc; fi
cargo_tool="${ONE_KVM_PLUGIN_CARGO:-cargo}"
target_dir="${CARGO_TARGET_DIR:-target}"
mkdir -p "$target_dir"
df -h "$target_dir"

packages=()
for plugin in "$@"; do
    case "$plugin" in
        rustdesk|vnc|rtsp|airferry) ;;
        easytier|easyfrp|frpc|gostc)
            case "$plugin" in
                easytier) variable=ONE_KVM_EASYTIER_CORE; tool=easytier; binary=easytier-core ;;
                easyfrp) variable=ONE_KVM_EASYFRPC; tool=easyfrp; binary=easyfrpc ;;
                frpc) variable=ONE_KVM_FRPC; tool=frp; binary=frpc ;;
                gostc) variable=ONE_KVM_GOSTC; tool=gostc; binary=gostc ;;
            esac
            if [[ -z "${!variable:-}" ]]; then
                case "$target" in
                    x86_64-*-linux-*) docker_platform=linux/amd64 ;;
                    aarch64-*-linux-*) docker_platform=linux/arm64 ;;
                    armv7-*-linux-*) docker_platform=linux/arm/v7 ;;
                    *)
                        echo "Set $variable to the $binary executable for $target." >&2
                        exit 1 ;;
                esac
                # Keep cross-build inputs visible under the source mount.
                asset_dir="build/.plugin-build/$target"
                python3 build/download-remote-tools.py --tool "$tool" --platform "$docker_platform" --output "$asset_dir"
                export "$variable=$asset_dir/$binary"
            fi
            [[ -s "${!variable}" ]] || { echo "$binary executable is missing or empty." >&2; exit 1; }
            ;;
        *) echo "Unknown plugin '$plugin'." >&2; exit 1 ;;
    esac
    packages+=(-p "one-kvm-plugin-$plugin")
done

"$cargo_tool" build --release --locked --target "$target" --target-dir "$target_dir" "${packages[@]}"
extension=
if [[ "$target" == *windows* ]]; then extension=.exe; fi
for plugin in "$@"; do
    python3 scripts/pack-plugin.py --manifest "plugins/$plugin/manifest.json" \
        --binary "$target_dir/$target/release/one-kvm-plugin-$plugin$extension" --target "$target" \
        --output "$target_dir/plugins/one-kvm-$plugin-$target.onepkg"
done
