# 宠安堂 · 宠物殡葬遗物封存与骨灰交付系统

门店在接运、服务和交付过程中多次移交遗物与骨灰，本系统记录每件物品的保管人链，
并让客户随时查询自己委托的办理进度。

## 一键启动

```bash
./start.sh
```

首次运行会安装前端依赖、构建前端并编译后端，之后访问 **http://localhost:8080**。

| 角色 | 账号 | 密码 |
| ---- | ---- | ---- |
| 门店员工 | `zhang` / `liu` | `staff123` |
| 客户 | `chen` / `wang` | `cust123` |

## 技术栈

- **后端**：Rust + Axum，SQLx 访问 SQLite（WAL 模式），照片存本地 `./photos`
- **前端**：Svelte 5 + Vite，构建产物由后端直接托管
- **通知**：模拟接口（`src/notify.rs`），仅落库留档不真正发送

## 业务规则

- **委托单**记录接运信息、服务项目、遗物清单与封签号；服务状态
  （待接运→已接运→服务中→服务已完成）与物品交付状态（未交付/已交付）相互独立。
- **移交签收**：每次移交生成待签收记录，仅指定接收人本人签收后保管人才变更。
- **服务清单版本**：客户每次确认生成不可变快照（`service_list_versions`），
  门店后续调整工作清单不会覆盖历史版本，历史版本永远可查。
- **交付守卫**（`src/handlers_delivery.rs` 的 `deliver`，单事务 `BEGIN IMMEDIATE`）：
  - 员工现场输入委托号复核，不一致拒绝；
  - 逐项核对每件遗物封签号 / 骨灰罐封罐号，不一致返回 409 并说明具体物品；
  - 本人领取核对客户档案证件号；代领须存在**未撤销**的授权且人证一致，
    已撤销授权返回 403 并说明原因；
  - 同一委托的未完成出库申请唯一（部分唯一索引），重复申请返回 409；
  - `deliveries.commission_id` 唯一约束 + 事务内状态检查，
    两名员工同时操作也只有一笔交付成功，另一笔收到 409。
- **客户隔离**：客户只能查看自己的委托（服务端强制过滤）。

## 验证

```bash
cargo test   # 8 个集成测试覆盖全部关键规则
```

- `wrong_seal_blocks_delivery` — 错误封罐号被拦截，委托保持未交付
- `revoked_proxy_blocks_delivery` — 撤销代领后交付被拦截
- `duplicate_outbound_blocked` — 重复出库申请被拦截；服务未完成不能申请出库
- `duplicate_delivery_blocked` / `concurrent_delivery_only_one_succeeds` — 重复与并发交付均只能成功一次
- `service_list_versions_preserved` — 历史确认版本不被后续变更覆盖且可查询
- `customer_isolation` — 客户数据隔离与越权拦截
- `transfer_requires_receiver_signature` — 移交必须由接收人本人签收

## 目录结构

```
migrations/0001_init.sql   数据库结构（含唯一约束与部分索引）
src/                       后端（lib.rs 装配路由，main.rs 启动）
  handlers_commission.rs   认证、委托单、服务清单版本
  handlers_items.rs        遗物/骨灰罐、照片上传、移交签收
  handlers_delivery.rs     代领授权、出库申请、交付守卫
tests/delivery_rules.rs    集成测试
frontend/src/pages/        Svelte 页面（列表/详情/领取核对）
start.sh                   一键启动
```
