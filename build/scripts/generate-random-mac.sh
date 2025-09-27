#!/bin/bash

# 为玩客云/玩客云Pro 平台生成 MAC 地址的一次性脚本
# 此脚本在首次开机时执行，为 eth0 网卡生成并应用基于 SN 的 MAC 地址，失败时回退到随机 MAC

set -e

NETWORK_CONFIG="/etc/systemd/network/99-eth0.network"
LOCK_FILE="/var/lib/kvmd/.mac-generated"
PLATFORM_FILE="/usr/share/kvmd/platform"
EFUSE_SYSFS_PATH=""
SN_PREFIX=""
SN_EXPECTED_LENGTH=13

# 按平台设置 EFUSE 与 SN 参数；未知平台时按 efuse 路径探测
detect_platform_params() {
    local platform=""
    if [ -f "$PLATFORM_FILE" ]; then
        platform=$(tr -d '\n' < "$PLATFORM_FILE")
    fi

    case "$platform" in
        onecloud)
            EFUSE_SYSFS_PATH="/sys/bus/nvmem/devices/meson8b-efuse0/nvmem"
            SN_PREFIX="OCP"
            ;;
        onecloud-pro)
            EFUSE_SYSFS_PATH="/sys/devices/platform/efuse/efuse0/nvmem"
            SN_PREFIX="ODC"
            ;;
    esac

    if [ -z "$EFUSE_SYSFS_PATH" ] || [ -z "$SN_PREFIX" ]; then
        if [ -e "/sys/devices/platform/efuse/efuse0/nvmem" ]; then
            EFUSE_SYSFS_PATH="/sys/devices/platform/efuse/efuse0/nvmem"
            SN_PREFIX="ODC"
        elif [ -e "/sys/bus/nvmem/devices/meson8b-efuse0/nvmem" ]; then
            EFUSE_SYSFS_PATH="/sys/bus/nvmem/devices/meson8b-efuse0/nvmem"
            SN_PREFIX="OCP"
        fi
    fi
}

# 检查是否已经执行过
if [ -f "$LOCK_FILE" ]; then
    echo "MAC地址已经生成过，跳过执行"
    exit 0
fi

# 生成MAC地址函数
generate_random_mac() {
    detect_platform_params
    # 尝试根据 SN 生成唯一 MAC 地址
    if [ -f "$EFUSE_SYSFS_PATH" ]; then
        sn_offset=$(grep --binary-files=text -boP "$SN_PREFIX" "$EFUSE_SYSFS_PATH" | head -n1 | cut -d: -f1)
        if [ -n "$sn_offset" ]; then
            sn=$(dd if="$EFUSE_SYSFS_PATH" bs=1 skip="$sn_offset" count="$SN_EXPECTED_LENGTH" 2>/dev/null)
            if [ ${#sn} -eq $SN_EXPECTED_LENGTH ]; then
                echo "S/N: $sn" >&2  # 输出到 stderr，避免干扰返回值
                # 使用 SN 的 SHA-256 哈希生成后 5 字节（避免多余管道）
                sn_hash=$(printf %s "$sn" | sha256sum | cut -d' ' -f1)
                # 直接用 Bash 子串获取哈希末 10 个字符并插入分隔符
                mac_hex=${sn_hash: -10}
                mac_suffix=$(printf "%s:%s:%s:%s:%s" "${mac_hex:0:2}" "${mac_hex:2:2}" "${mac_hex:4:2}" "${mac_hex:6:2}" "${mac_hex:8:2}")
                printf "02:%s\n" "$mac_suffix"
                return 0
            fi
        fi
    fi

    # 若 SN 获取失败，回退到随机逻辑
    echo "警告: 无法获取 SN，回退到随机 MAC 生成" >&2
    printf "02:%02x:%02x:%02x:%02x:%02x\n" \
        $((RANDOM % 256)) \
        $((RANDOM % 256)) \
        $((RANDOM % 256)) \
        $((RANDOM % 256)) \
        $((RANDOM % 256))
}

echo "正在生成基于 SN 的 MAC 地址..."

# 生成新的MAC地址
NEW_MAC=$(generate_random_mac)
echo "生成的MAC地址: $NEW_MAC"

# 验证 MAC 地址格式
if ! [[ $NEW_MAC =~ ^([0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}$ ]]; then
    echo "错误: 生成的 MAC 地址格式无效: $NEW_MAC"
    exit 1
fi

# 备份原配置文件
if [ -f "$NETWORK_CONFIG" ]; then
    cp "$NETWORK_CONFIG" "${NETWORK_CONFIG}.backup"
fi

# 更新网络配置文件
cat > "$NETWORK_CONFIG" << EOF
[Match]
Name=eth0

[Network]
DHCP=yes

[Link]
MACAddress=$NEW_MAC
EOF

echo "已更新网络配置文件: $NETWORK_CONFIG"

# 创建锁定文件，防止重复执行
mkdir -p "$(dirname "$LOCK_FILE")"
echo "MAC地址生成时间: $(date)" > "$LOCK_FILE"

# 禁用此服务，确保只运行一次
systemctl disable kvmd-generate-mac.service

echo "MAC地址生成完成: $NEW_MAC"
echo "服务已自动禁用，下次开机不会再执行"

exit 0