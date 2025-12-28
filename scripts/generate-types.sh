#!/bin/bash
# 从 Rust 生成 TypeScript 类型定义
# 使用 typeshare-cli 工具

set -e

# 获取项目根目录
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_FILE="$PROJECT_ROOT/web/src/types/generated.ts"

echo "Generating TypeScript types from Rust..."
echo "Project root: $PROJECT_ROOT"
echo "Output file: $OUTPUT_FILE"

# 确保输出目录存在
mkdir -p "$(dirname "$OUTPUT_FILE")"

# 运行 typeshare
typeshare "$PROJECT_ROOT/src" \
  --lang=typescript \
  --output-file="$OUTPUT_FILE"

echo ""
echo "TypeScript types generated successfully!"
echo "Output: $OUTPUT_FILE"
echo ""
echo "Generated types:"
grep -E "^export (interface|type|enum)" "$OUTPUT_FILE" | head -30 || true
