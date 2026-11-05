#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

arch="${GOARCH:-amd64}"
out="../bin/okvm-win-agent-${arch}.exe"

GOOS=windows GOARCH="$arch" CGO_ENABLED=0 \
  go build -trimpath -ldflags "-s -w" -o "$out" .

echo "Built $out"
