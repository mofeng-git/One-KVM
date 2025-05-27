#!/bin/bash

# KVMD首次运行初始化脚本
# 在首次开机时执行KVMD服务启动前的必要初始化操作

set -e

LOCK_FILE="/var/lib/kvmd/.kvmd-firstrun-completed"

# 检查是否已经执行过
[ -f "$LOCK_FILE" ] && { echo "[KVMD-FirstRun] 初始化已完成，跳过执行"; exit 0; }

echo "[KVMD-FirstRun] 开始KVMD首次运行初始化..."

# 1. 生成KVMD主证书
echo "[KVMD-FirstRun] 生成KVMD主证书..."
kvmd-gencert --do-the-thing

# 2. 生成VNC证书
echo "[KVMD-FirstRun] 生成VNC证书..."
kvmd-gencert --do-the-thing --vnc

# 3. 生成nginx配置文件
echo "[KVMD-FirstRun] 生成nginx配置文件..."
kvmd-nginx-mkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf || echo "[KVMD-FirstRun] 警告: nginx配置生成失败"

# 创建锁定文件
mkdir -p "$(dirname "$LOCK_FILE")"
echo "KVMD首次运行初始化完成 - $(date)" > "$LOCK_FILE"

# 禁用服务
systemctl disable kvmd-firstrun.service || echo "[KVMD-FirstRun] 警告: 服务禁用失败"

echo "[KVMD-FirstRun] 初始化完成！" 