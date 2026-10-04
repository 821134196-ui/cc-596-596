-- 宠物殡葬遗物封存与骨灰交付系统 — 数据库结构

CREATE TABLE IF NOT EXISTS users (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    username    TEXT NOT NULL UNIQUE,
    password    TEXT NOT NULL,               -- sha256(盐+密码)，演示用途
    role        TEXT NOT NULL CHECK (role IN ('staff', 'customer')),
    name        TEXT NOT NULL,
    phone       TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE IF NOT EXISTS sessions (
    token       TEXT PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id),
    created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 委托单：接运信息内联在委托单上，服务项目/遗物/封签为子表
CREATE TABLE IF NOT EXISTS commissions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    code             TEXT NOT NULL UNIQUE,          -- 委托号，如 WT-20261004-0001
    customer_id      INTEGER NOT NULL REFERENCES users(id),
    pet_name         TEXT NOT NULL,
    pet_species      TEXT NOT NULL DEFAULT '',
    -- 接运信息
    pickup_address   TEXT NOT NULL DEFAULT '',
    pickup_time      TEXT NOT NULL DEFAULT '',
    pickup_contact   TEXT NOT NULL DEFAULT '',
    pickup_notes     TEXT NOT NULL DEFAULT '',
    -- 骨灰封罐号（交付核对用）
    urn_seal_no      TEXT NOT NULL DEFAULT '',
    -- 两个独立状态：服务进度 与 物品交付
    service_status   TEXT NOT NULL DEFAULT 'in_progress'
                     CHECK (service_status IN ('in_progress', 'completed')),
    delivery_status  TEXT NOT NULL DEFAULT 'pending'
                     CHECK (delivery_status IN ('pending', 'delivered')),
    created_at       TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 当前（可编辑）服务项目清单
CREATE TABLE IF NOT EXISTS service_items (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions(id),
    name          TEXT NOT NULL,
    price_cents   INTEGER NOT NULL DEFAULT 0,
    note          TEXT NOT NULL DEFAULT ''
);

-- 客户确认后的服务清单版本（不可变快照，后续变更不得覆盖）
CREATE TABLE IF NOT EXISTS service_versions (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions(id),
    version_no    INTEGER NOT NULL,
    confirmed_by  TEXT NOT NULL,               -- 确认人（客户姓名）
    confirmed_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE (commission_id, version_no)
);

CREATE TABLE IF NOT EXISTS service_version_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    version_id  INTEGER NOT NULL REFERENCES service_versions(id),
    name        TEXT NOT NULL,
    price_cents INTEGER NOT NULL DEFAULT 0,
    note        TEXT NOT NULL DEFAULT ''
);

-- 遗物清单：holder 记录当前保管人
CREATE TABLE IF NOT EXISTS relics (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions(id),
    name          TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    photo_path    TEXT NOT NULL DEFAULT '',
    holder        TEXT NOT NULL DEFAULT '',    -- 当前保管人
    created_at    TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 封签
CREATE TABLE IF NOT EXISTS seals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions(id),
    seal_no       TEXT NOT NULL,
    item_desc     TEXT NOT NULL DEFAULT '',    -- 封于何物（遗物/骨灰罐/档案袋…）
    sealed_by     TEXT NOT NULL DEFAULT '',
    sealed_at     TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE (commission_id, seal_no)
);

-- 移交记录：每次移交由接收人签收
CREATE TABLE IF NOT EXISTS transfers (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions(id),
    item_label    TEXT NOT NULL,               -- 移交物品（遗物名/骨灰罐/…）
    from_holder   TEXT NOT NULL,
    to_holder     TEXT NOT NULL,
    signed_by     TEXT NOT NULL,               -- 签收人（接收人签名）
    signed_at     TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    note          TEXT NOT NULL DEFAULT ''
);

-- 代领授权
CREATE TABLE IF NOT EXISTS proxies (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL REFERENCES commissions(id),
    name          TEXT NOT NULL,
    id_no         TEXT NOT NULL DEFAULT '',    -- 证件号
    phone         TEXT NOT NULL DEFAULT '',
    status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'revoked')),
    created_at    TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 交付记录：commission_id 唯一约束保证同一委托只能成功出库一次
CREATE TABLE IF NOT EXISTS deliveries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER NOT NULL UNIQUE REFERENCES commissions(id),
    receiver_name TEXT NOT NULL,
    receiver_type TEXT NOT NULL CHECK (receiver_type IN ('customer', 'proxy')),
    receiver_id_no TEXT NOT NULL DEFAULT '',
    urn_seal_no   TEXT NOT NULL,               -- 交付时核对录入的封罐号
    delivered_by  TEXT NOT NULL,               -- 经办员工
    delivered_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 模拟通知（站内 outbox，真实环境可替换为短信/微信网关）
CREATE TABLE IF NOT EXISTS notifications (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    commission_id INTEGER REFERENCES commissions(id),
    recipient     TEXT NOT NULL,
    content       TEXT NOT NULL,
    sent_at       TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX IF NOT EXISTS idx_commissions_customer ON commissions(customer_id);
CREATE INDEX IF NOT EXISTS idx_transfers_commission ON transfers(commission_id);
CREATE INDEX IF NOT EXISTS idx_notifications_commission ON notifications(commission_id);
