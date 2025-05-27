#!/bin/bash

# 为onecloud平台生成随机MAC地址的一次性脚本
# 此脚本在首次开机时执行，为eth0网卡生成并应用随机MAC地址

set -e

NETWORK_CONFIG="/etc/systemd/network/99-eth0.network"
LOCK_FILE="/var/lib/kvmd/.mac-generated"

# 检查是否已经执行过
if [ -f "$LOCK_FILE" ]; then
    echo "MAC地址已经生成过，跳过执行"
    exit 0
fi

# 生成随机MAC地址 (使用本地管理的MAC地址前缀)
generate_random_mac() {
    # 使用本地管理的MAC地址前缀 (第二位设为2、6、A、E中的一个)
    # 这样可以避免与真实硬件MAC地址冲突
    printf "02:%02x:%02x:%02x:%02x:%02x\n" \
        $((RANDOM % 256)) \
        $((RANDOM % 256)) \
        $((RANDOM % 256)) \
        $((RANDOM % 256)) \
        $((RANDOM % 256))
}

echo "正在为onecloud生成随机MAC地址..."

# 生成新的MAC地址
NEW_MAC=$(generate_random_mac)
echo "生成的MAC地址: $NEW_MAC"

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

echo "随机MAC地址生成完成: $NEW_MAC"
echo "服务已自动禁用，下次开机不会再执行"

exit 0 