use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth::{hash_password, AuthUser};
use crate::error::ApiError;
use crate::models::*;
use crate::AppState;

// ---------- 模拟通知：写入 outbox 并打印日志 ----------

async fn notify(pool: &SqlitePool, commission_id: Option<i64>, recipient: &str, content: &str) {
    let _ = sqlx::query(
        "INSERT INTO notifications (commission_id, recipient, content) VALUES (?,?,?)",
    )
    .bind(commission_id)
    .bind(recipient)
    .bind(content)
    .execute(pool)
    .await;
    println!("[notify/mock] -> {recipient}: {content}");
}

// ---------- 认证 ----------

#[derive(Deserialize)]
pub struct LoginReq {
    username: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<Json<Value>, ApiError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, password, role, name, phone FROM users WHERE username = ?",
    )
    .bind(&req.username)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::unauthorized("用户名或密码错误"))?;
    if user.password != hash_password(&req.password) {
        return Err(ApiError::unauthorized("用户名或密码错误"));
    }
    let token = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions (token, user_id) VALUES (?,?)")
        .bind(&token)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "token": token, "user": user })))
}

pub async fn me(auth: AuthUser) -> Json<Value> {
    Json(json!({ "user": auth.0 }))
}

pub async fn logout(State(state): State<AppState>, auth: AuthUser) -> Result<Json<Value>, ApiError> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(auth.0.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------- 员工：客户与委托 ----------

#[derive(Deserialize)]
pub struct CreateCommissionReq {
    customer_username: String,
    customer_name: Option<String>,
    customer_phone: Option<String>,
    pet_name: String,
    pet_species: Option<String>,
    pickup_address: Option<String>,
    pickup_time: Option<String>,
    pickup_contact: Option<String>,
    pickup_notes: Option<String>,
}

pub async fn list_commissions(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    let rows = sqlx::query_as::<_, (i64, String, String, String, String, String, String, String)>(
        "SELECT c.id, c.code, c.pet_name, c.pet_species, c.service_status, c.delivery_status, c.created_at, u.name \
         FROM commissions c JOIN users u ON u.id = c.customer_id ORDER BY c.id DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    let list: Vec<Value> = rows
        .into_iter()
        .map(|(id, code, pet_name, pet_species, service_status, delivery_status, created_at, customer_name)| {
            json!({
                "id": id, "code": code, "pet_name": pet_name, "pet_species": pet_species,
                "service_status": service_status, "delivery_status": delivery_status,
                "created_at": created_at, "customer_name": customer_name,
            })
        })
        .collect();
    Ok(Json(json!({ "commissions": list })))
}

pub async fn create_commission(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateCommissionReq>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    if req.pet_name.trim().is_empty() {
        return Err(ApiError::bad_request("宠物名字不能为空"));
    }
    // 查找或创建客户账号（默认密码 123456）
    let customer = sqlx::query_as::<_, User>(
        "SELECT id, username, password, role, name, phone FROM users WHERE username = ?",
    )
    .bind(&req.customer_username)
    .fetch_optional(&state.pool)
    .await?;
    let customer_id = match customer {
        Some(u) => {
            if u.role != "customer" {
                return Err(ApiError::bad_request("该用户名不是客户账号"));
            }
            u.id
        }
        None => {
            let name = req.customer_name.clone().unwrap_or_else(|| req.customer_username.clone());
            let phone = req.customer_phone.clone().unwrap_or_default();
            let r = sqlx::query(
                "INSERT INTO users (username, password, role, name, phone) VALUES (?,?,?,?,?)",
            )
            .bind(&req.customer_username)
            .bind(hash_password("123456"))
            .bind("customer")
            .bind(&name)
            .bind(&phone)
            .execute(&state.pool)
            .await?;
            r.last_insert_rowid()
        }
    };

    let mut tx = state.pool.begin().await?;
    let r = sqlx::query(
        "INSERT INTO commissions (code, customer_id, pet_name, pet_species, pickup_address, pickup_time, pickup_contact, pickup_notes) \
         VALUES ('', ?,?,?,?,?,?,?)",
    )
    .bind(customer_id)
    .bind(req.pet_name.trim())
    .bind(req.pet_species.clone().unwrap_or_default())
    .bind(req.pickup_address.clone().unwrap_or_default())
    .bind(req.pickup_time.clone().unwrap_or_default())
    .bind(req.pickup_contact.clone().unwrap_or_default())
    .bind(req.pickup_notes.clone().unwrap_or_default())
    .execute(&mut *tx)
    .await?;
    let id = r.last_insert_rowid();
    let code = format!(
        "WT-{}-{:04}",
        chrono::Local::now().format("%Y%m%d"),
        id
    );
    sqlx::query("UPDATE commissions SET code = ? WHERE id = ?")
        .bind(&code)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    notify(&state.pool, Some(id), &req.customer_username,
           &format!("委托 {code} 已受理，可随时登录查询办理进度")).await;
    Ok(Json(json!({ "id": id, "code": code })))
}

#[derive(Deserialize)]
pub struct UpdatePickupReq {
    pickup_address: String,
    pickup_time: String,
    pickup_contact: String,
    pickup_notes: Option<String>,
    urn_seal_no: Option<String>,
}

pub async fn update_pickup(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdatePickupReq>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    let r = sqlx::query(
        "UPDATE commissions SET pickup_address=?, pickup_time=?, pickup_contact=?, pickup_notes=?, urn_seal_no=? WHERE id=?",
    )
    .bind(&req.pickup_address)
    .bind(&req.pickup_time)
    .bind(&req.pickup_contact)
    .bind(req.pickup_notes.clone().unwrap_or_default())
    .bind(req.urn_seal_no.clone().unwrap_or_default())
    .bind(id)
    .execute(&state.pool)
    .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::not_found("委托不存在"));
    }
    Ok(Json(json!({ "ok": true })))
}

// ---------- 服务项目与客户确认版本 ----------

#[derive(Deserialize)]
pub struct ServiceItemIn {
    name: String,
    price_cents: Option<i64>,
    note: Option<String>,
}

#[derive(Deserialize)]
pub struct SetItemsReq {
    items: Vec<ServiceItemIn>,
}

pub async fn set_service_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<SetItemsReq>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    ensure_commission_exists(&state.pool, id).await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM service_items WHERE commission_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for it in &req.items {
        if it.name.trim().is_empty() {
            continue;
        }
        sqlx::query("INSERT INTO service_items (commission_id, name, price_cents, note) VALUES (?,?,?,?)")
            .bind(id)
            .bind(it.name.trim())
            .bind(it.price_cents.unwrap_or(0))
            .bind(it.note.clone().unwrap_or_default())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ConfirmReq {
    confirmed_by: String,
}

/// 客户确认服务清单：把当前清单固化为一个不可变的新版本
pub async fn confirm_service_list(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<ConfirmReq>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    if req.confirmed_by.trim().is_empty() {
        return Err(ApiError::bad_request("请填写确认人（客户）姓名"));
    }
    let items = sqlx::query_as::<_, ServiceItem>(
        "SELECT * FROM service_items WHERE commission_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    if items.is_empty() {
        return Err(ApiError::bad_request("当前服务清单为空，无法确认"));
    }
    let mut tx = state.pool.begin().await?;
    let next_no: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(version_no), 0) + 1 FROM service_versions WHERE commission_id = ?",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let r = sqlx::query(
        "INSERT INTO service_versions (commission_id, version_no, confirmed_by) VALUES (?,?,?)",
    )
    .bind(id)
    .bind(next_no.0)
    .bind(req.confirmed_by.trim())
    .execute(&mut *tx)
    .await?;
    let version_id = r.last_insert_rowid();
    for it in &items {
        sqlx::query("INSERT INTO service_version_items (version_id, name, price_cents, note) VALUES (?,?,?,?)")
            .bind(version_id)
            .bind(&it.name)
            .bind(it.price_cents)
            .bind(&it.note)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    let c = get_commission(&state.pool, id).await?;
    notify(&state.pool, Some(id), &format!("customer#{}", c.customer_id),
           &format!("委托 {} 的服务清单已确认（第 {} 版），历史版本可随时查阅", c.code, next_no.0)).await;
    Ok(Json(json!({ "ok": true, "version_no": next_no.0 })))
}

// ---------- 遗物 / 封签 / 移交 ----------

#[derive(Deserialize)]
pub struct AddRelicReq {
    name: String,
    description: Option<String>,
    holder: Option<String>,
}

pub async fn add_relic(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<AddRelicReq>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    ensure_commission_exists(&state.pool, id).await?;
    if req.name.trim().is_empty() {
        return Err(ApiError::bad_request("遗物名称不能为空"));
    }
    let r = sqlx::query("INSERT INTO relics (commission_id, name, description, holder) VALUES (?,?,?,?)")
        .bind(id)
        .bind(req.name.trim())
        .bind(req.description.clone().unwrap_or_default())
        .bind(req.holder.clone().unwrap_or_else(|| "门店".into()))
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "id": r.last_insert_rowid() })))
}

/// 遗物照片上传：存本地 uploads/ 目录
pub async fn upload_relic_photo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((cid, rid)): Path<(i64, i64)>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    let relic = sqlx::query_as::<_, Relic>("SELECT * FROM relics WHERE id = ? AND commission_id = ?")
        .bind(rid)
        .bind(cid)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("遗物不存在"))?;
    let _ = relic;
    while let Some(field) = multipart.next_field().await.map_err(ApiError::internal)? {
        if field.name() != Some("file") {
            continue;
        }
        let orig = field.file_name().unwrap_or("photo").to_string();
        let ext = orig.rsplit('.').next().unwrap_or("bin")
            .chars().filter(|c| c.is_ascii_alphanumeric()).take(8).collect::<String>();
        let ext = if ext.is_empty() { "bin".into() } else { ext };
        let data = field.bytes().await.map_err(ApiError::internal)?;
        if data.len() > 10 * 1024 * 1024 {
            return Err(ApiError::bad_request("照片不能超过 10MB"));
        }
        let fname = format!("relic-{rid}-{}.{}", Uuid::new_v4(), ext);
        let path = std::path::Path::new(&state.upload_dir).join(&fname);
        tokio::fs::write(&path, &data).await?;
        let rel = format!("/uploads/{fname}");
        sqlx::query("UPDATE relics SET photo_path = ? WHERE id = ?")
            .bind(&rel)
            .bind(rid)
            .execute(&state.pool)
            .await?;
        return Ok(Json(json!({ "photo_path": rel })));
    }
    Err(ApiError::bad_request("未收到文件"))
}

#[derive(Deserialize)]
pub struct AddSealReq {
    seal_no: String,
    item_desc: Option<String>,
    sealed_by: Option<String>,
}

pub async fn add_seal(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<AddSealReq>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    ensure_commission_exists(&state.pool, id).await?;
    if req.seal_no.trim().is_empty() {
        return Err(ApiError::bad_request("封签号不能为空"));
    }
    let r = sqlx::query(
        "INSERT INTO seals (commission_id, seal_no, item_desc, sealed_by) VALUES (?,?,?,?)",
    )
    .bind(id)
    .bind(req.seal_no.trim())
    .bind(req.item_desc.clone().unwrap_or_default())
    .bind(req.sealed_by.clone().unwrap_or_else(|| auth.0.name.clone()))
    .execute(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            ApiError::conflict("SEAL_EXISTS", "该封签号在此委托下已存在")
        } else {
            ApiError::internal(e)
        }
    })?;
    Ok(Json(json!({ "id": r.last_insert_rowid() })))
}

#[derive(Deserialize)]
pub struct AddTransferReq {
    item_label: String,
    from_holder: String,
    to_holder: String,
    signed_by: String,
    note: Option<String>,
}

/// 移交登记：接收人签收；若移交物品是已登记遗物，同步更新保管人
pub async fn add_transfer(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<AddTransferReq>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    ensure_commission_exists(&state.pool, id).await?;
    if req.item_label.trim().is_empty() || req.to_holder.trim().is_empty() || req.signed_by.trim().is_empty() {
        return Err(ApiError::bad_request("移交物品、接收人、签收人均不能为空"));
    }
    let mut tx = state.pool.begin().await?;
    let r = sqlx::query(
        "INSERT INTO transfers (commission_id, item_label, from_holder, to_holder, signed_by, note) VALUES (?,?,?,?,?,?)",
    )
    .bind(id)
    .bind(req.item_label.trim())
    .bind(req.from_holder.trim())
    .bind(req.to_holder.trim())
    .bind(req.signed_by.trim())
    .bind(req.note.clone().unwrap_or_default())
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE relics SET holder = ? WHERE commission_id = ? AND name = ?")
        .bind(req.to_holder.trim())
        .bind(id)
        .bind(req.item_label.trim())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": r.last_insert_rowid() })))
}

// ---------- 代领授权 ----------

#[derive(Deserialize)]
pub struct AddProxyReq {
    name: String,
    id_no: Option<String>,
    phone: Option<String>,
}

pub async fn add_proxy(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<AddProxyReq>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    ensure_commission_exists(&state.pool, id).await?;
    if req.name.trim().is_empty() {
        return Err(ApiError::bad_request("代领人姓名不能为空"));
    }
    let r = sqlx::query("INSERT INTO proxies (commission_id, name, id_no, phone) VALUES (?,?,?,?)")
        .bind(id)
        .bind(req.name.trim())
        .bind(req.id_no.clone().unwrap_or_default())
        .bind(req.phone.clone().unwrap_or_default())
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "id": r.last_insert_rowid() })))
}

pub async fn revoke_proxy(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((cid, pid)): Path<(i64, i64)>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    let r = sqlx::query(
        "UPDATE proxies SET status = 'revoked' WHERE id = ? AND commission_id = ? AND status = 'active'",
    )
    .bind(pid)
    .bind(cid)
    .execute(&state.pool)
    .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::not_found("授权不存在或已撤销"));
    }
    Ok(Json(json!({ "ok": true })))
}

// ---------- 服务完成 / 交付 ----------

pub async fn complete_service(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    let c = get_commission(&state.pool, id).await?;
    if c.service_status == "completed" {
        return Err(ApiError::conflict("SERVICE_ALREADY_COMPLETED", "服务已标记完成"));
    }
    sqlx::query("UPDATE commissions SET service_status = 'completed' WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?;
    notify(&state.pool, Some(id), &format!("customer#{}", c.customer_id),
           &format!("委托 {} 的服务已完成，可预约到店领取骨灰与遗物", c.code)).await;
    Ok(Json(json!({ "ok": true })))
}

/// 领取页数据（员工）：故意不返回 urn_seal_no —— 封罐号必须由员工从实物读取录入
pub async fn delivery_info(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(code): Path<String>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    let c = sqlx::query_as::<_, Commission>("SELECT * FROM commissions WHERE code = ?")
        .bind(&code)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("委托号不存在，请核对后重试"))?;
    let customer = sqlx::query_as::<_, User>(
        "SELECT id, username, password, role, name, phone FROM users WHERE id = ?",
    )
    .bind(c.customer_id)
    .fetch_one(&state.pool)
    .await?;
    let proxies = sqlx::query_as::<_, Proxy>(
        "SELECT * FROM proxies WHERE commission_id = ? ORDER BY id",
    )
    .bind(c.id)
    .fetch_all(&state.pool)
    .await?;
    let delivery = sqlx::query_as::<_, Delivery>(
        "SELECT * FROM deliveries WHERE commission_id = ?",
    )
    .bind(c.id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(Json(json!({
        "commission": {
            "id": c.id, "code": c.code, "pet_name": c.pet_name, "pet_species": c.pet_species,
            "service_status": c.service_status, "delivery_status": c.delivery_status,
        },
        "customer": { "name": customer.name, "phone": customer.phone },
        "proxies": proxies,
        "delivery": delivery,
    })))
}

#[derive(Deserialize)]
pub struct DeliverReq {
    commission_code: String,
    urn_seal_no: String,
    receiver_type: String, // customer | proxy
    receiver_name: String,
    receiver_id_no: Option<String>,
}

/// 骨灰/遗物出库交付。拦截规则（均返回 409 + 具体原因）：
///   1. 服务未完成            SERVICE_NOT_COMPLETED
///   2. 封罐号核对不一致       SEAL_MISMATCH
///   3. 领取人非委托人本人     RECEIVER_NOT_OWNER
///   4. 无此代领授权           PROXY_NOT_FOUND
///   5. 代领授权已撤销         PROXY_REVOKED
///   6. 重复出库（含并发）     ALREADY_DELIVERED —— 由 deliveries.commission_id 唯一约束兜底
pub async fn deliver(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<DeliverReq>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    if req.urn_seal_no.trim().is_empty() || req.receiver_name.trim().is_empty() {
        return Err(ApiError::bad_request("封罐号与领取人姓名不能为空"));
    }

    // BEGIN IMMEDIATE：事务开始即获取写锁。并发交付时后到的事务排队等待（busy_timeout），
    // 而非在延迟事务升级写锁时直接报 SQLITE_BUSY_SNAPSHOT。
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await
        .map_err(ApiError::internal)?;
    let c = sqlx::query_as::<_, Commission>("SELECT * FROM commissions WHERE code = ?")
        .bind(req.commission_code.trim())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::not_found("委托号不存在"))?;

    // 1. 服务必须先完成
    if c.service_status != "completed" {
        return Err(ApiError::conflict(
            "SERVICE_NOT_COMPLETED",
            format!("委托 {} 的服务尚未完成，不能交付", c.code),
        ));
    }
    // 2. 已交付则拒绝（快速路径；并发场景由下方唯一约束兜底）
    if c.delivery_status == "delivered" {
        let d = sqlx::query_as::<_, Delivery>("SELECT * FROM deliveries WHERE commission_id = ?")
            .bind(c.id)
            .fetch_optional(&mut *tx)
            .await?;
        let detail = d
            .map(|d| format!("（已于 {} 由 {} 经办交付给 {}）", d.delivered_at, d.delivered_by, d.receiver_name))
            .unwrap_or_default();
        return Err(ApiError::conflict(
            "ALREADY_DELIVERED",
            format!("重复出库：委托 {} 的物品已交付{detail}", c.code),
        ));
    }
    // 3. 封罐号核对
    if c.urn_seal_no.is_empty() {
        return Err(ApiError::conflict(
            "SEAL_NOT_SET",
            "该委托尚未登记封罐号，请先在委托详情中登记",
        ));
    }
    if c.urn_seal_no != req.urn_seal_no.trim() {
        return Err(ApiError::conflict(
            "SEAL_MISMATCH",
            "封签核对不一致：录入的封罐号与委托登记不符，交付已阻止",
        ));
    }
    // 4. 领取人资格
    match req.receiver_type.as_str() {
        "customer" => {
            let owner = sqlx::query_as::<_, (String,)>("SELECT name FROM users WHERE id = ?")
                .bind(c.customer_id)
                .fetch_one(&mut *tx)
                .await?;
            if owner.0 != req.receiver_name.trim() {
                return Err(ApiError::conflict(
                    "RECEIVER_NOT_OWNER",
                    format!("领取人与委托客户不符：本委托客户为「{}」", owner.0),
                ));
            }
        }
        "proxy" => {
            let p = sqlx::query_as::<_, Proxy>(
                "SELECT * FROM proxies WHERE commission_id = ? AND name = ?",
            )
            .bind(c.id)
            .bind(req.receiver_name.trim())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| {
                ApiError::conflict("PROXY_NOT_FOUND", "无此代领授权：该领取人不在本委托的代领名单中")
            })?;
            if p.status == "revoked" {
                return Err(ApiError::conflict(
                    "PROXY_REVOKED",
                    format!("代领授权已撤销：「{}」的代领资格已被取消，交付已阻止", p.name),
                ));
            }
        }
        _ => return Err(ApiError::bad_request("receiver_type 必须为 customer 或 proxy")),
    }

    // 5. 写入交付记录 —— commission_id 唯一约束保证并发下只有一单成功
    let insert = sqlx::query(
        "INSERT INTO deliveries (commission_id, receiver_name, receiver_type, receiver_id_no, urn_seal_no, delivered_by) \
         VALUES (?,?,?,?,?,?)",
    )
    .bind(c.id)
    .bind(req.receiver_name.trim())
    .bind(&req.receiver_type)
    .bind(req.receiver_id_no.clone().unwrap_or_default())
    .bind(req.urn_seal_no.trim())
    .bind(&auth.0.name)
    .execute(&mut *tx)
    .await;
    if let Err(e) = insert {
        if e.to_string().contains("UNIQUE") {
            return Err(ApiError::conflict(
                "ALREADY_DELIVERED",
                format!("重复出库：委托 {} 已被另一笔操作交付，本次已阻止", c.code),
            ));
        }
        return Err(ApiError::internal(e));
    }
    let updated = sqlx::query(
        "UPDATE commissions SET delivery_status = 'delivered' WHERE id = ? AND delivery_status = 'pending'",
    )
    .bind(c.id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::conflict(
            "ALREADY_DELIVERED",
            format!("重复出库：委托 {} 已被另一笔操作交付，本次已阻止", c.code),
        ));
    }
    tx.commit().await?;

    notify(&state.pool, Some(c.id), &format!("customer#{}", c.customer_id),
           &format!("委托 {} 的骨灰与遗物已交付给 {}，感谢信任", c.code, req.receiver_name.trim())).await;
    Ok(Json(json!({ "ok": true, "commission_code": c.code })))
}

// ---------- 详情查询（员工全量 / 客户限本人） ----------

async fn ensure_commission_exists(pool: &SqlitePool, id: i64) -> Result<(), ApiError> {
    get_commission(pool, id).await.map(|_| ())
}

async fn get_commission(pool: &SqlitePool, id: i64) -> Result<Commission, ApiError> {
    sqlx::query_as::<_, Commission>("SELECT * FROM commissions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("委托不存在"))
}

async fn commission_detail(pool: &SqlitePool, c: &Commission) -> Result<Value, ApiError> {
    let customer = sqlx::query_as::<_, (String, String)>(
        "SELECT name, phone FROM users WHERE id = ?",
    )
    .bind(c.customer_id)
    .fetch_one(pool)
    .await?;
    let items = sqlx::query_as::<_, ServiceItem>(
        "SELECT * FROM service_items WHERE commission_id = ? ORDER BY id",
    ).bind(c.id).fetch_all(pool).await?;
    let versions = sqlx::query_as::<_, ServiceVersion>(
        "SELECT * FROM service_versions WHERE commission_id = ? ORDER BY version_no",
    ).bind(c.id).fetch_all(pool).await?;
    let mut versions_with_items = Vec::new();
    for v in versions {
        let vitems = sqlx::query_as::<_, ServiceVersionItem>(
            "SELECT * FROM service_version_items WHERE version_id = ? ORDER BY id",
        ).bind(v.id).fetch_all(pool).await?;
        versions_with_items.push(json!({ "version": v, "items": vitems }));
    }
    let relics = sqlx::query_as::<_, Relic>(
        "SELECT * FROM relics WHERE commission_id = ? ORDER BY id",
    ).bind(c.id).fetch_all(pool).await?;
    let seals = sqlx::query_as::<_, Seal>(
        "SELECT * FROM seals WHERE commission_id = ? ORDER BY id",
    ).bind(c.id).fetch_all(pool).await?;
    let transfers = sqlx::query_as::<_, Transfer>(
        "SELECT * FROM transfers WHERE commission_id = ? ORDER BY id",
    ).bind(c.id).fetch_all(pool).await?;
    let proxies = sqlx::query_as::<_, Proxy>(
        "SELECT * FROM proxies WHERE commission_id = ? ORDER BY id",
    ).bind(c.id).fetch_all(pool).await?;
    let delivery = sqlx::query_as::<_, Delivery>(
        "SELECT * FROM deliveries WHERE commission_id = ?",
    ).bind(c.id).fetch_optional(pool).await?;
    let notifications = sqlx::query_as::<_, Notification>(
        "SELECT * FROM notifications WHERE commission_id = ? ORDER BY id",
    ).bind(c.id).fetch_all(pool).await?;
    Ok(json!({
        "commission": c,
        "customer": { "name": customer.0, "phone": customer.1 },
        "items": items,
        "versions": versions_with_items,
        "relics": relics,
        "seals": seals,
        "transfers": transfers,
        "proxies": proxies,
        "delivery": delivery,
        "notifications": notifications,
    }))
}

pub async fn staff_commission_detail(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    auth.require_staff()?;
    let c = get_commission(&state.pool, id).await?;
    Ok(Json(commission_detail(&state.pool, &c).await?))
}

// ---------- 客户：仅本人委托 ----------

pub async fn my_commissions(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Value>, ApiError> {
    if auth.0.role != "customer" {
        return Err(ApiError::forbidden("仅客户账号可访问"));
    }
    let rows = sqlx::query_as::<_, Commission>(
        "SELECT * FROM commissions WHERE customer_id = ? ORDER BY id DESC",
    )
    .bind(auth.0.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "commissions": rows })))
}

pub async fn my_commission_detail(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    if auth.0.role != "customer" {
        return Err(ApiError::forbidden("仅客户账号可访问"));
    }
    let c = get_commission(&state.pool, id).await?;
    if c.customer_id != auth.0.id {
        // 客户只能查看自己的委托
        return Err(ApiError::forbidden("无权查看他人的委托"));
    }
    Ok(Json(commission_detail(&state.pool, &c).await?))
}
