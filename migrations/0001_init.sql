-- 宠物殡葬遗物封存与骨灰交付系统 数据库结构

CREATE TABLE users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL CHECK (role IN ('staff', 'customer')),
    name          TEXT NOT NULL,
    phone         TEXT NOT NULL DEFAULT '',
    id_no         TEXT NOT NULL DEFAULT '',          -- 证件号，交付时核对领取人身份
    created_at    TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE sessions (
    token      TEXT PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users (id),
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 委托单：接运信息 + 服务状态 + 交付状态（两者明确分离）
CREATE TABLE commissions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_no    TEXT NOT NULL UNIQUE,           -- 委托号，如 WT20261004-0001
    customer_id      INTEGER NOT NULL REFERENCES users (id),
    pet_name         TEXT NOT NULL,
    pet_species      TEXT NOT NULL DEFAULT '',
    -- 接运信息
    pickup_address   TEXT NOT NULL DEFAULT '',
    pickup_contact   TEXT NOT NULL DEFAULT '',
    pickup_phone     TEXT NOT NULL DEFAULT '',
    pickup_time      TEXT NOT NULL DEFAULT '',
    pickup_staff_id  INTEGER REFERENCES users (id),
    pickup_notes     TEXT NOT NULL DEFAULT '',
    -- 服务进度：pending 待接运 / picked_up 已接运 / in_service 服务中 / completed 服务已完成
    service_status   TEXT NOT NULL DEFAULT 'pending'
                     CHECK (service_status IN ('pending', 'picked_up', 'in_service', 'completed')),
    -- 物品交付：none 未交付 / delivered 已交付（与服务状态独立）
    delivery_status  TEXT NOT NULL DEFAULT 'none'
                     CHECK (delivery_status IN ('none', 'delivered')),
    created_at       TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 服务项目（当前工作清单，可由门店调整）
CREATE TABLE service_items (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions (id),
    name          TEXT NOT NULL,
    price_cents   INTEGER NOT NULL DEFAULT 0,
    note          TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 客户确认过的服务清单版本：独立保存、不可覆盖、不可修改
CREATE TABLE service_list_versions (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions (id),
    version_no    INTEGER NOT NULL,
    items_json    TEXT NOT NULL,                     -- 确认时刻的清单快照
    total_cents   INTEGER NOT NULL DEFAULT 0,
    confirmed_by  INTEGER NOT NULL REFERENCES users (id),
    confirmed_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE (commission_id, version_no)
);

-- 遗物与骨灰罐：每件都有唯一封签号（骨灰罐即封罐号）
CREATE TABLE items (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id     INTEGER NOT NULL REFERENCES commissions (id),
    item_type         TEXT NOT NULL CHECK (item_type IN ('relic', 'urn')),
    name              TEXT NOT NULL,
    description       TEXT NOT NULL DEFAULT '',
    seal_no           TEXT NOT NULL,                 -- 封签号 / 封罐号
    photo_path        TEXT NOT NULL DEFAULT '',
    status            TEXT NOT NULL DEFAULT 'in_store'
                      CHECK (status IN ('in_store', 'in_transit', 'delivered')),
    current_holder_id INTEGER REFERENCES users (id), -- 当前保管人
    created_at        TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE (commission_id, seal_no)
);

-- 移交记录：每次移交由接收人签收后才生效
CREATE TABLE transfers (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id  INTEGER NOT NULL REFERENCES commissions (id),
    item_id        INTEGER NOT NULL REFERENCES items (id),
    from_holder    TEXT NOT NULL DEFAULT '',         -- 交出人名称快照
    to_holder_id   INTEGER REFERENCES users (id),    -- 接收人（员工/客户账号）
    to_holder_name TEXT NOT NULL,                    -- 接收人名称快照（交付时可为代领人）
    transfer_type  TEXT NOT NULL,                    -- pickup 接运 / intake 入店 / service 服务 / storage 封存 / delivery 交付
    status         TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'signed')),
    signed_by      INTEGER REFERENCES users (id),    -- 签收人
    signed_at      TEXT,
    notes          TEXT NOT NULL DEFAULT '',
    created_at     TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 代领授权：客户授权他人代领，可撤销
CREATE TABLE proxy_authorizations (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions (id),
    proxy_name    TEXT NOT NULL,
    proxy_id_no   TEXT NOT NULL,
    proxy_phone   TEXT NOT NULL DEFAULT '',
    status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'revoked')),
    created_by    INTEGER NOT NULL REFERENCES users (id),
    created_at    TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    revoked_at    TEXT
);

-- 出库申请：同一委托同一时间只允许一个未完成的出库申请
CREATE TABLE outbound_requests (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions (id),
    requested_by  INTEGER NOT NULL REFERENCES users (id),
    status        TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'fulfilled', 'cancelled')),
    created_at    TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE UNIQUE INDEX uniq_open_outbound ON outbound_requests (commission_id) WHERE status = 'open';

-- 交付记录：每个委托最多交付一次（并发守卫的唯一约束）
CREATE TABLE deliveries (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id       INTEGER NOT NULL UNIQUE REFERENCES commissions (id),
    outbound_request_id INTEGER REFERENCES outbound_requests (id),
    recipient_type      TEXT NOT NULL CHECK (recipient_type IN ('self', 'proxy')),
    recipient_name      TEXT NOT NULL,
    recipient_id_no     TEXT NOT NULL,
    proxy_auth_id       INTEGER REFERENCES proxy_authorizations (id),
    seal_checks_json    TEXT NOT NULL,               -- 逐项封签核对结果留档
    delivered_by        INTEGER NOT NULL REFERENCES users (id),
    delivered_at        TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 模拟通知（不真正发送，仅记录）
CREATE TABLE notifications (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER REFERENCES commissions (id),
    recipient     TEXT NOT NULL,
    channel       TEXT NOT NULL DEFAULT 'sms',
    content       TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'mock_sent',
    created_at    TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
