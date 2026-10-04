#!/usr/bin/env bash
# 一条命令启动整个项目：构建前端 → 构建后端 → 启动服务（端口 8080）
set -euo pipefail
cd "$(dirname "$0")"

export PATH="$HOME/.cargo/bin:$PATH"

echo "==> [1/3] 构建前端 (Svelte + Vite)"
(cd web && npm install --no-fund --no-audit --silent && npm run build)

echo "==> [2/3] 构建后端 (Rust + Axum)"
(cd server && cargo build)

echo "==> [3/3] 启动服务"
echo "    打开 http://localhost:8080"
echo "    员工账号 staff/staff123 · 客户账号 chen/123456, liu/123456"
cd server
exec ./target/debug/petmemorial-server
