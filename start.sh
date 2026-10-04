#!/usr/bin/env bash
# 宠安堂 · 宠物殡葬遗物封存与骨灰交付系统 —— 一键启动
set -euo pipefail
cd "$(dirname "$0")"

export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "错误：未找到 Rust 工具链，请先安装 https://rustup.rs/" >&2
  exit 1
fi
if ! command -v npm >/dev/null 2>&1; then
  echo "错误：未找到 Node.js/npm，请先安装 Node.js 18+" >&2
  exit 1
fi

# 首次运行或设置 REBUILD=1 时重新构建前端
if [ ! -f frontend/dist/index.html ] || [ "${REBUILD:-0}" = "1" ]; then
  echo "==> 构建前端…"
  (cd frontend && npm install && npm run build)
fi

echo "==> 启动后端（首次运行需编译，请稍候）…"
echo "    访问地址: http://localhost:${PORT:-8080}"
echo "    演示账号  员工: zhang/staff123, liu/staff123  客户: chen/cust123, wang/cust123"
exec cargo run
