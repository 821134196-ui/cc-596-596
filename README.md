# 宠物殡葬 · 遗物封存与骨灰交付系统

门店在接运、服务和交付过程中多次移交物品，系统全程记录每件遗物与骨灰的保管人；
客户可在线查询自己委托的办理进度；交付环节逐项核对并对异常出库强制拦截。

## 快速开始

```bash
./start.sh          # 构建前端 + 后端并启动，访问 http://localhost:8080
```

演示账号：员工 `staff/staff123`；客户 `chen/123456`、`liu/123456`（新建客户默认密码 `123456`）。

运行验收测试（另开一个终端，服务保持运行）：

```bash
node scripts/verify.mjs
```

## 技术栈

- **后端**：Rust + Axum + SQLx（SQLite，WAL 模式），`server/`
- **前端**：Svelte 5 + Vite，构建产物由后端静态托管，`web/`
- **照片**：本地 `server/uploads/`，通过 `/uploads/...` 访问
- **通知**：模拟接口——写入 `notifications` 表并打印服务端日志（`[notify/mock]`），真实环境可替换为短信/微信网关

## 业务规则实现

| 需求 | 实现 |
|---|---|
| 委托单记录接运信息/服务项目/遗物清单/封签 | `commissions` + `service_items`/`relics`/`seals` 子表 |
| 每次移交由接收人签收 | `transfers` 表记录签收人；移交遗物时同步更新其当前保管人 |
| 客户确认的服务清单不可覆盖 | `service_versions` + `service_version_items` 不可变快照，每次确认生成新版本号 |
| 领取页逐项核对 | 员工须勾选委托号、录入实物封罐号（页面不显示登记值，防照抄）、核对领取人后才能提交 |
| 服务完成 ≠ 物品交付 | `service_status` 与 `delivery_status` 两个独立状态 |
| 客户只能查看自己的委托 | `/api/my/*` 接口强制 `customer_id = 当前用户`，越权返回 403 |
| 错误封签拦截 | 交付时比对 `urn_seal_no`，不一致返回 `409 SEAL_MISMATCH` |
| 撤销代领拦截 | 代领人状态 `revoked` 时返回 `409 PROXY_REVOKED`；无授权返回 `PROXY_NOT_FOUND` |
| 重复出库拦截（含并发） | `deliveries.commission_id` 唯一约束兜底 + `BEGIN IMMEDIATE` 事务；后到请求返回 `409 ALREADY_DELIVERED` |

所有拦截响应均为 `409` 并携带 `{ error: { code, message } }`，`message` 说明具体原因，前端领取页直接展示。

## 目录结构

```
start.sh                  一键启动
scripts/verify.mjs        端到端验收（25 项断言）
server/
  migrations/0001_init.sql  数据库结构（唯一约束在此）
  src/main.rs               路由与启动
  src/handlers.rs           业务处理（deliver 为交付拦截核心）
  src/auth.rs / models.rs / db.rs / error.rs
web/
  src/pages/staff/          委托列表、委托详情、领取交付核对页
  src/pages/customer/       我的委托、办理进度
```

## 环境变量（均有默认值）

- `DATABASE_URL`（默认 `sqlite:data/app.db`）
- `UPLOAD_DIR`（默认 `uploads`）
- `WEB_DIST`（默认 `../web/dist`）
- `BIND`（默认 `0.0.0.0:8080`）
