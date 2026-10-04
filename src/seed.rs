use sqlx::SqlitePool;

use crate::auth::hash_password;

async fn insert_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    role: &str,
    name: &str,
    phone: &str,
    id_no: &str,
) -> anyhow::Result<i64> {
    let row = sqlx::query(
        "INSERT INTO users (username, password_hash, role, name, phone, id_no)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(username) DO UPDATE SET username = excluded.username
         RETURNING id",
    )
    .bind(username)
    .bind(hash_password(password))
    .bind(role)
    .bind(name)
    .bind(phone)
    .bind(id_no)
    .fetch_one(pool)
    .await?;
    use sqlx::Row;
    Ok(row.get::<i64, _>("id"))
}

/// 初始化种子账号与一条演示委托；幂等，可重复执行
pub async fn seed(pool: &SqlitePool) -> anyhow::Result<()> {
    let staff_zhang = insert_user(pool, "zhang", "staff123", "staff", "张雯", "13900000001", "").await?;
    let _staff_liu = insert_user(pool, "liu", "staff123", "staff", "刘洋", "13900000002", "").await?;
    let customer_chen = insert_user(
        pool,
        "chen",
        "cust123",
        "customer",
        "陈静",
        "13800000001",
        "110101199001011234",
    )
    .await?;
    let _customer_wang = insert_user(
        pool,
        "wang",
        "cust123",
        "customer",
        "王强",
        "13800000002",
        "110101198505052345",
    )
    .await?;

    // 演示委托：若已存在则跳过
    let exists: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM commissions WHERE commission_no = 'WT20261001-0001'")
            .fetch_optional(pool)
            .await?;
    if exists.is_some() {
        return Ok(());
    }

    let commission_id: i64 = sqlx::query_scalar(
        "INSERT INTO commissions
           (commission_no, customer_id, pet_name, pet_species,
            pickup_address, pickup_contact, pickup_phone, pickup_time, pickup_staff_id,
            pickup_notes, service_status)
         VALUES ('WT20261001-0001', ?, '豆豆', '金毛犬',
                 '朝阳区幸福里小区 3 号楼 2 单元', '陈静', '13800000001', '2026-10-01 10:00', ?,
                 '客户要求上门接运，宠物已在家中离世', 'in_service')
         RETURNING id",
    )
    .bind(customer_chen)
    .bind(staff_zhang)
    .fetch_one(pool)
    .await?;

    // 服务项目（工作清单）
    for (name, price, note) in [
        ("遗体接运", 30000, "专车上门"),
        ("遗体清洁与告别仪式", 88000, "独立告别厅"),
        ("单独火化", 120000, "全程可视"),
        ("骨灰罐（青瓷）", 36000, "含刻字"),
    ] {
        sqlx::query(
            "INSERT INTO service_items (commission_id, name, price_cents, note) VALUES (?, ?, ?, ?)",
        )
        .bind(commission_id)
        .bind(name)
        .bind(price)
        .bind(note)
        .execute(pool)
        .await?;
    }

    // 客户已确认的服务清单版本 v1（不可变快照）
    let items_snapshot = serde_json::json!([
        {"name": "遗体接运", "price_cents": 30000, "note": "专车上门"},
        {"name": "遗体清洁与告别仪式", "price_cents": 88000, "note": "独立告别厅"},
        {"name": "单独火化", "price_cents": 120000, "note": "全程可视"},
        {"name": "骨灰罐（青瓷）", "price_cents": 36000, "note": "含刻字"},
    ]);
    sqlx::query(
        "INSERT INTO service_list_versions
           (commission_id, version_no, items_json, total_cents, confirmed_by)
         VALUES (?, 1, ?, 274000, ?)",
    )
    .bind(commission_id)
    .bind(items_snapshot.to_string())
    .bind(customer_chen)
    .execute(pool)
    .await?;

    // 遗物与骨灰罐
    let relic_id: i64 = sqlx::query_scalar(
        "INSERT INTO items (commission_id, item_type, name, description, seal_no, current_holder_id)
         VALUES (?, 'relic', '项圈与铭牌', '红色皮质项圈，含铜质铭牌', 'FS-0001-A', ?)
         RETURNING id",
    )
    .bind(commission_id)
    .bind(staff_zhang)
    .fetch_one(pool)
    .await?;
    let urn_id: i64 = sqlx::query_scalar(
        "INSERT INTO items (commission_id, item_type, name, description, seal_no, current_holder_id)
         VALUES (?, 'urn', '骨灰罐（青瓷）', '罐身刻字：豆豆 2018-2026', 'FG-0001-A', ?)
         RETURNING id",
    )
    .bind(commission_id)
    .bind(staff_zhang)
    .fetch_one(pool)
    .await?;

    // 两条已签收的移交记录（接运入店）
    for (item_id, ttype) in [(relic_id, "pickup"), (urn_id, "intake")] {
        sqlx::query(
            "INSERT INTO transfers
               (commission_id, item_id, from_holder, to_holder_id, to_holder_name,
                transfer_type, status, signed_by, signed_at, notes)
             VALUES (?, ?, '陈静（客户）', ?, '张雯', ?, 'signed', ?, datetime('now','localtime'), '现场清点无误')",
        )
        .bind(commission_id)
        .bind(item_id)
        .bind(staff_zhang)
        .bind(ttype)
        .bind(staff_zhang)
        .execute(pool)
        .await?;
    }

    crate::notify::send_mock(
        pool,
        Some(commission_id),
        "13800000001",
        "【宠安堂】您的委托 WT20261001-0001 已受理，服务进行中，可随时查询进度。",
    )
    .await;

    Ok(())
}
