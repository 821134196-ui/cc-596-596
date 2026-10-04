use axum::{extract::Path, Json};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::auth::{create_session, hash_password, AuthUser, LoginReq, LoginResp, StaffUser, User};
use crate::error::ApiError;
use crate::models::*;
use crate::AppState;

// ---------- 认证 ----------

pub async fn login(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<Json<LoginResp>, ApiError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, role, name, phone, id_no FROM users WHERE username = ?",
    )
    .bind(&req.username)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::Unauthorized("用户名或密码错误".into()))?;
    if user.password_hash != hash_password(&req.password) {
        return Err(ApiError::Unauthorized("用户名或密码错误".into()));
    }
    let token = create_session(&state.pool, user.id).await?;
    Ok(Json(LoginResp { token, user }))
}

pub async fn me(AuthUser(user): AuthUser) -> Json<User> {
    Json(user)
}

pub async fn logout(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Value>, ApiError> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({"ok": true})))
}

// ---------- 公共工具 ----------

pub async fn load_commission(pool: &SqlitePool, id: i64) -> Result<Commission, ApiError> {
    sqlx::query_as::<_, Commission>("SELECT * FROM commissions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("委托单 {id} 不存在")))
}

/// 客户只能访问自己的委托；员工可访问全部
pub fn check_access(user: &User, commission: &Commission) -> Result<(), ApiError> {
    if user.is_staff() || commission.customer_id == user.id {
        Ok(())
    } else {
        Err(ApiError::forbidden("无权查看他人的委托单"))
    }
}

// ---------- 委托单 ----------

pub async fn list_commissions(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Value>, ApiError> {
    let rows = if user.is_staff() {
        sqlx::query_as::<_, Commission>("SELECT * FROM commissions ORDER BY id DESC")
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query_as::<_, Commission>(
            "SELECT * FROM commissions WHERE customer_id = ? ORDER BY id DESC",
        )
        .bind(user.id)
        .fetch_all(&state.pool)
        .await?
    };
    // 附带客户姓名，便于列表展示
    let mut out = Vec::new();
    for c in rows {
        let customer_name: String =
            sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
                .bind(c.customer_id)
                .fetch_one(&state.pool)
                .await?;
        out.push(json!({
            "commission": c,
            "customer_name": customer_name,
        }));
    }
    Ok(Json(json!(out)))
}

pub async fn create_commission(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(staff): StaffUser,
    Json(req): Json<CreateCommissionReq>,
) -> Result<Json<Value>, ApiError> {
    if req.customer_username.trim().is_empty() || req.pet_name.trim().is_empty() {
        return Err(ApiError::bad_request("客户账号与宠物名称不能为空"));
    }
    // 客户账号不存在则自动创建（默认密码 123456，首次登录后应修改）
    let customer: User = match sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, role, name, phone, id_no FROM users WHERE username = ?",
    )
    .bind(req.customer_username.trim())
    .fetch_optional(&state.pool)
    .await?
    {
        Some(u) if u.role == "customer" => u,
        Some(_) => return Err(ApiError::bad_request("该用户名已被员工账号占用")),
        None => {
            let id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO users (username, password_hash, role, name, phone, id_no)
                 VALUES (?, ?, 'customer', ?, ?, ?) RETURNING id",
            )
            .bind(req.customer_username.trim())
            .bind(hash_password("123456"))
            .bind(req.customer_name.trim())
            .bind(req.customer_phone.clone().unwrap_or_default())
            .bind(req.customer_id_no.clone().unwrap_or_default())
            .fetch_one(&state.pool)
            .await?;
            sqlx::query_as::<_, User>(
                "SELECT id, username, password_hash, role, name, phone, id_no FROM users WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&state.pool)
            .await?
        }
    };

    let today = chrono_lite_today();
    let seq: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) + 1 FROM commissions WHERE commission_no LIKE ?",
    )
    .bind(format!("WT{today}-%"))
    .fetch_one(&state.pool)
    .await?;
    let commission_no = format!("WT{today}-{seq:04}");

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO commissions
           (commission_no, customer_id, pet_name, pet_species,
            pickup_address, pickup_contact, pickup_phone, pickup_time, pickup_staff_id, pickup_notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&commission_no)
    .bind(customer.id)
    .bind(req.pet_name.trim())
    .bind(req.pet_species.clone().unwrap_or_default())
    .bind(req.pickup_address.clone().unwrap_or_default())
    .bind(req.pickup_contact.clone().unwrap_or_default())
    .bind(req.pickup_phone.clone().unwrap_or_default())
    .bind(req.pickup_time.clone().unwrap_or_default())
    .bind(staff.id)
    .bind(req.pickup_notes.clone().unwrap_or_default())
    .fetch_one(&state.pool)
    .await?;

    crate::notify::send_mock(
        &state.pool,
        Some(id),
        &customer.phone,
        &format!("【宠安堂】您的委托 {commission_no} 已建立，我们将悉心照护。"),
    )
    .await;

    let commission = load_commission(&state.pool, id).await?;
    Ok(Json(json!({"commission": commission})))
}

/// 简易日期：取 created_at 风格当天日期（避免引入 chrono 依赖）
fn chrono_lite_today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // UTC+8
    let secs = now + 8 * 3600;
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}{m:02}{d:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub async fn get_commission(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    check_access(&user, &commission)?;
    let customer_name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
        .bind(commission.customer_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"commission": commission, "customer_name": customer_name})))
}

pub async fn update_pickup(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(_staff): StaffUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdatePickupReq>,
) -> Result<Json<Value>, ApiError> {
    load_commission(&state.pool, id).await?;
    sqlx::query(
        "UPDATE commissions SET pickup_address=?, pickup_contact=?, pickup_phone=?, pickup_time=?,
            pickup_notes=?, updated_at=datetime('now','localtime') WHERE id=?",
    )
    .bind(&req.pickup_address)
    .bind(&req.pickup_contact)
    .bind(&req.pickup_phone)
    .bind(&req.pickup_time)
    .bind(req.pickup_notes.clone().unwrap_or_default())
    .bind(id)
    .execute(&state.pool)
    .await?;
    let commission = load_commission(&state.pool, id).await?;
    Ok(Json(json!({"commission": commission})))
}

pub async fn update_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(_staff): StaffUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdateStatusReq>,
) -> Result<Json<Value>, ApiError> {
    const ORDER: [&str; 4] = ["pending", "picked_up", "in_service", "completed"];
    let commission = load_commission(&state.pool, id).await?;
    let cur = ORDER.iter().position(|s| *s == commission.service_status).unwrap_or(0);
    let next = ORDER.iter().position(|s| *s == req.service_status);
    let Some(next) = next else {
        return Err(ApiError::bad_request("非法的服务状态"));
    };
    if next != cur + 1 {
        return Err(ApiError::conflict(format!(
            "服务状态只能按顺序推进：当前「{}」，不能直接变为「{}」",
            commission.service_status, req.service_status
        )));
    }
    sqlx::query(
        "UPDATE commissions SET service_status=?, updated_at=datetime('now','localtime') WHERE id=?",
    )
    .bind(&req.service_status)
    .bind(id)
    .execute(&state.pool)
    .await?;
    if req.service_status == "completed" {
        let phone: String = sqlx::query_scalar(
            "SELECT u.phone FROM users u JOIN commissions c ON c.customer_id=u.id WHERE c.id=?",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
        crate::notify::send_mock(
            &state.pool,
            Some(id),
            &phone,
            &format!("【宠安堂】委托 {} 的服务已完成，可预约到店领取。", commission.commission_no),
        )
        .await;
    }
    let commission = load_commission(&state.pool, id).await?;
    Ok(Json(json!({"commission": commission})))
}

// ---------- 服务项目与确认版本 ----------

pub async fn list_service_items(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    check_access(&user, &commission)?;
    let items = sqlx::query_as::<_, ServiceItem>(
        "SELECT * FROM service_items WHERE commission_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(items)))
}

pub async fn add_service_item(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(_staff): StaffUser,
    Path(id): Path<i64>,
    Json(req): Json<AddServiceItemReq>,
) -> Result<Json<Value>, ApiError> {
    load_commission(&state.pool, id).await?;
    if req.name.trim().is_empty() {
        return Err(ApiError::bad_request("服务项目名称不能为空"));
    }
    let item_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO service_items (commission_id, name, price_cents, note) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(id)
    .bind(req.name.trim())
    .bind(req.price_cents.unwrap_or(0))
    .bind(req.note.clone().unwrap_or_default())
    .fetch_one(&state.pool)
    .await?;
    let item = sqlx::query_as::<_, ServiceItem>("SELECT * FROM service_items WHERE id = ?")
        .bind(item_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"item": item})))
}

pub async fn delete_service_item(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(_staff): StaffUser,
    Path(item_id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    sqlx::query("DELETE FROM service_items WHERE id = ?")
        .bind(item_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({"ok": true})))
}

/// 客户确认当前服务清单 → 生成不可变版本快照；后续对工作清单的修改不会覆盖历史版本
pub async fn confirm_service_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    if user.is_staff() {
        return Err(ApiError::forbidden("服务清单需由客户本人确认"));
    }
    if commission.customer_id != user.id {
        return Err(ApiError::forbidden("只能确认自己的委托"));
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
    let snapshot: Vec<Value> = items
        .iter()
        .map(|i| json!({"name": i.name, "price_cents": i.price_cents, "note": i.note}))
        .collect();
    let total: i64 = items.iter().map(|i| i.price_cents).sum();
    let version_no: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version_no), 0) + 1 FROM service_list_versions WHERE commission_id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    let vid = sqlx::query_scalar::<_, i64>(
        "INSERT INTO service_list_versions (commission_id, version_no, items_json, total_cents, confirmed_by)
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(id)
    .bind(version_no)
    .bind(serde_json::to_string(&snapshot).unwrap())
    .bind(total)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    let version = sqlx::query_as::<_, ServiceListVersion>(
        "SELECT * FROM service_list_versions WHERE id = ?",
    )
    .bind(vid)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({"version": version})))
}

/// 历史确认版本列表：任何时刻都可查询，不受后续变更影响
pub async fn list_service_versions(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    check_access(&user, &commission)?;
    let versions = sqlx::query_as::<_, ServiceListVersion>(
        "SELECT * FROM service_list_versions WHERE commission_id = ? ORDER BY version_no",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(versions)))
}

// ---------- 员工列表（用于移交时选择接收人） ----------

pub async fn list_staff(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(_staff): StaffUser,
) -> Result<Json<Value>, ApiError> {
    let staff = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, username, name FROM users WHERE role = 'staff' ORDER BY id",
    )
    .fetch_all(&state.pool)
    .await?;
    let out: Vec<Value> = staff
        .iter()
        .map(|(id, username, name)| json!({"id": id, "username": username, "name": name}))
        .collect();
    Ok(Json(json!(out)))
}
