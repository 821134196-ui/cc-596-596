use axum::{extract::Path, Json};
use serde_json::{json, Value};

use crate::auth::{AuthUser, StaffUser};
use crate::error::ApiError;
use crate::handlers_commission::{check_access, load_commission};
use crate::models::*;
use crate::AppState;

// ---------- 代领授权 ----------

pub async fn list_proxy(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    check_access(&user, &commission)?;
    let rows = sqlx::query_as::<_, ProxyAuth>(
        "SELECT * FROM proxy_authorizations WHERE commission_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(rows)))
}

pub async fn create_proxy(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<CreateProxyReq>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    if user.is_staff() || commission.customer_id != user.id {
        return Err(ApiError::forbidden("只有委托客户本人可以登记代领授权"));
    }
    if commission.delivery_status == "delivered" {
        return Err(ApiError::conflict("该委托已交付，不能再登记代领授权"));
    }
    if req.proxy_name.trim().is_empty() || req.proxy_id_no.trim().is_empty() {
        return Err(ApiError::bad_request("代领人姓名与证件号不能为空"));
    }
    let pid = sqlx::query_scalar::<_, i64>(
        "INSERT INTO proxy_authorizations (commission_id, proxy_name, proxy_id_no, proxy_phone, created_by)
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(id)
    .bind(req.proxy_name.trim())
    .bind(req.proxy_id_no.trim())
    .bind(req.proxy_phone.clone().unwrap_or_default())
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    let proxy = sqlx::query_as::<_, ProxyAuth>("SELECT * FROM proxy_authorizations WHERE id = ?")
        .bind(pid)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"proxy": proxy})))
}

pub async fn revoke_proxy(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(proxy_id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let proxy = sqlx::query_as::<_, ProxyAuth>("SELECT * FROM proxy_authorizations WHERE id = ?")
        .bind(proxy_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("代领授权不存在"))?;
    let commission = load_commission(&state.pool, proxy.commission_id).await?;
    if user.is_staff() || commission.customer_id != user.id {
        return Err(ApiError::forbidden("只有委托客户本人可以撤销代领授权"));
    }
    if proxy.status == "revoked" {
        return Err(ApiError::conflict("该代领授权已撤销，无需重复操作"));
    }
    sqlx::query(
        "UPDATE proxy_authorizations SET status='revoked', revoked_at=datetime('now','localtime') WHERE id=?",
    )
    .bind(proxy_id)
    .execute(&state.pool)
    .await?;
    let proxy = sqlx::query_as::<_, ProxyAuth>("SELECT * FROM proxy_authorizations WHERE id = ?")
        .bind(proxy_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"proxy": proxy})))
}

// ---------- 出库申请 ----------

pub async fn list_outbound(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    check_access(&user, &commission)?;
    let rows = sqlx::query_as::<_, OutboundRequest>(
        "SELECT * FROM outbound_requests WHERE commission_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(rows)))
}

/// 申请出库：同一委托存在未完成申请时必须拒绝
pub async fn create_outbound(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(staff): StaffUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    if commission.delivery_status == "delivered" {
        return Err(ApiError::conflict("该委托的物品已交付，不能再申请出库"));
    }
    if commission.service_status != "completed" {
        return Err(ApiError::conflict("服务尚未完成，不能申请出库"));
    }
    let req_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO outbound_requests (commission_id, requested_by) VALUES (?, ?) RETURNING id",
    )
    .bind(id)
    .bind(staff.id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            ApiError::conflict(format!(
                "委托 {} 已存在未完成的出库申请，不能重复申请出库",
                commission.commission_no
            ))
        } else {
            ApiError::from(e)
        }
    })?;
    let outbound = sqlx::query_as::<_, OutboundRequest>(
        "SELECT * FROM outbound_requests WHERE id = ?",
    )
    .bind(req_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({"outbound": outbound})))
}

// ---------- 交付（核心守卫） ----------

/// 领取交付：逐项核对委托号、封签号、领取人；
/// 代领授权撤销 / 封签不一致 / 重复交付 一律拒绝并说明原因；
/// BEGIN IMMEDIATE + UNIQUE(commission_id) 保证并发下仅一次成功。
pub async fn deliver(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(staff): StaffUser,
    Path(id): Path<i64>,
    Json(req): Json<DeliverReq>,
) -> Result<Json<Value>, ApiError> {
    let mut conn = state.pool.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    let result = deliver_in_tx(&mut conn, &state, &staff, id, &req).await;
    match result {
        Ok(value) => {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            // 通知在事务外发送（模拟接口，不参与一致性）
            let commission = load_commission(&state.pool, id).await?;
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
                &format!(
                    "【宠安堂】委托 {} 的遗物与骨灰已交付{}，感谢信任。",
                    commission.commission_no, req.recipient_name
                ),
            )
            .await;
            Ok(Json(value))
        }
        Err(e) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            Err(e)
        }
    }
}

async fn deliver_in_tx(
    conn: &mut sqlx::SqliteConnection,
    state: &AppState,
    staff: &crate::auth::User,
    id: i64,
    req: &DeliverReq,
) -> Result<Value, ApiError> {
    // 1. 委托存在 + 员工现场输入的委托号必须一致（防拿错）
    let commission = sqlx::query_as::<_, Commission>("SELECT * FROM commissions WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("委托单 {id} 不存在")))?;
    if req.commission_no.trim() != commission.commission_no {
        return Err(ApiError::bad_request(format!(
            "委托号核对不一致：输入「{}」与委托单「{}」不符",
            req.commission_no.trim(),
            commission.commission_no
        )));
    }

    // 2. 服务必须已完成（服务完成与物品交付是两个独立状态）
    if commission.service_status != "completed" {
        return Err(ApiError::conflict(format!(
            "委托 {} 的服务尚未完成（当前状态：{}），不能交付物品",
            commission.commission_no, commission.service_status
        )));
    }

    // 3. 不能重复交付
    if commission.delivery_status == "delivered" {
        return Err(ApiError::conflict(format!(
            "委托 {} 的物品已完成交付，不能重复交付",
            commission.commission_no
        )));
    }

    // 4. 出库申请：指定了则必须有效；未指定则自动关联当前未完成申请
    let mut outbound_id = req.outbound_request_id;
    if let Some(oid) = outbound_id {
        let ob = sqlx::query_as::<_, OutboundRequest>(
            "SELECT * FROM outbound_requests WHERE id = ?",
        )
        .bind(oid)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| ApiError::bad_request("出库申请不存在"))?;
        if ob.commission_id != id {
            return Err(ApiError::bad_request("出库申请不属于该委托"));
        }
        if ob.status != "open" {
            return Err(ApiError::conflict("该出库申请已处理，不能重复使用"));
        }
    } else {
        outbound_id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM outbound_requests WHERE commission_id = ? AND status = 'open'",
        )
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    }

    // 5. 领取人身份核对
    let customer = sqlx::query_as::<_, (String, String)>(
        "SELECT u.name, u.id_no FROM users u JOIN commissions c ON c.customer_id = u.id WHERE c.id = ?",
    )
    .bind(id)
    .fetch_one(&mut *conn)
    .await?;
    let mut proxy_auth_id: Option<i64> = None;
    match req.recipient_type.as_str() {
        "self" => {
            if req.recipient_name.trim() != customer.0 {
                return Err(ApiError::forbidden(format!(
                    "领取人「{}」与委托客户「{}」不一致",
                    req.recipient_name.trim(),
                    customer.0
                )));
            }
            if !customer.1.is_empty() && req.recipient_id_no.trim() != customer.1 {
                return Err(ApiError::forbidden("领取人证件号与客户档案不一致"));
            }
        }
        "proxy" => {
            let pid = req
                .proxy_auth_id
                .ok_or_else(|| ApiError::bad_request("代领交付必须选择代领授权记录"))?;
            let proxy = sqlx::query_as::<_, ProxyAuth>(
                "SELECT * FROM proxy_authorizations WHERE id = ?",
            )
            .bind(pid)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or_else(|| ApiError::bad_request("代领授权记录不存在"))?;
            if proxy.commission_id != id {
                return Err(ApiError::bad_request("代领授权不属于该委托"));
            }
            if proxy.status == "revoked" {
                return Err(ApiError::forbidden(format!(
                    "代领授权已被客户撤销（代领人：{}），禁止交付",
                    proxy.proxy_name
                )));
            }
            if req.recipient_name.trim() != proxy.proxy_name
                || req.recipient_id_no.trim() != proxy.proxy_id_no
            {
                return Err(ApiError::forbidden(
                    "现场领取人信息与代领授权登记不一致，禁止交付",
                ));
            }
            proxy_auth_id = Some(pid);
        }
        other => return Err(ApiError::bad_request(format!("未知的领取人类型: {other}"))),
    }

    // 6. 逐项封签核对：委托下每件物品都必须核对且一致
    let items = sqlx::query_as::<_, Item>(
        "SELECT * FROM items WHERE commission_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    if items.is_empty() {
        return Err(ApiError::conflict("该委托没有登记任何遗物或骨灰罐，不能交付"));
    }
    let mut check_results = Vec::new();
    for item in &items {
        let label = if item.item_type == "urn" { "封罐号" } else { "封签号" };
        let check = req.seal_checks.iter().find(|c| c.item_id == item.id);
        match check {
            None => {
                return Err(ApiError::bad_request(format!(
                    "缺少物品「{}」的{label}核对，请逐项核对后再交付",
                    item.name
                )))
            }
            Some(c) if c.seal_no.trim() != item.seal_no => {
                return Err(ApiError::conflict(format!(
                    "物品「{}」{label}核对不一致（填写：{}），交付已阻止",
                    item.name,
                    c.seal_no.trim()
                )))
            }
            Some(c) => check_results.push(json!({
                "item_id": item.id, "item_name": item.name,
                "seal_no": c.seal_no.trim(), "matched": true,
            })),
        }
    }
    for c in &req.seal_checks {
        if !items.iter().any(|i| i.id == c.item_id) {
            return Err(ApiError::bad_request(format!(
                "封签核对包含不属于该委托的物品 id={}",
                c.item_id
            )));
        }
    }

    // 7. 写入交付记录：UNIQUE(commission_id) 是并发下的最终防线
    let delivery_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO deliveries
           (commission_id, outbound_request_id, recipient_type, recipient_name, recipient_id_no,
            proxy_auth_id, seal_checks_json, delivered_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(id)
    .bind(outbound_id)
    .bind(&req.recipient_type)
    .bind(req.recipient_name.trim())
    .bind(req.recipient_id_no.trim())
    .bind(proxy_auth_id)
    .bind(serde_json::to_string(&check_results).unwrap())
    .bind(staff.id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            ApiError::conflict(format!(
                "委托 {} 已完成交付，不能重复交付",
                commission.commission_no
            ))
        } else {
            ApiError::from(e)
        }
    })?;

    // 8. 状态推进：委托交付状态、物品状态、出库申请、交付移交记录
    sqlx::query(
        "UPDATE commissions SET delivery_status='delivered', updated_at=datetime('now','localtime') WHERE id=?",
    )
    .bind(id)
    .execute(&mut *conn)
    .await?;
    sqlx::query("UPDATE items SET status='delivered' WHERE commission_id=?")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    if let Some(oid) = outbound_id {
        sqlx::query("UPDATE outbound_requests SET status='fulfilled' WHERE id=?")
            .bind(oid)
            .execute(&mut *conn)
            .await?;
    }
    for item in &items {
        let from_name: String = match item.current_holder_id {
            Some(h) => sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
                .bind(h)
                .fetch_optional(&mut *conn)
                .await?
                .unwrap_or_default(),
            None => String::new(),
        };
        sqlx::query(
            "INSERT INTO transfers
               (commission_id, item_id, from_holder, to_holder_id, to_holder_name,
                transfer_type, status, signed_by, signed_at, notes)
             VALUES (?, ?, ?, NULL, ?, 'delivery', 'signed', ?, datetime('now','localtime'), '交付签收')",
        )
        .bind(id)
        .bind(item.id)
        .bind(&from_name)
        .bind(req.recipient_name.trim())
        .bind(staff.id)
        .execute(&mut *conn)
        .await?;
    }

    let _ = state; // 事务内仅使用连接
    Ok(json!({
        "delivery_id": delivery_id,
        "commission_no": commission.commission_no,
        "delivered_items": items.len(),
    }))
}

pub async fn get_delivery(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    check_access(&user, &commission)?;
    let delivery = sqlx::query_as::<_, Delivery>(
        "SELECT * FROM deliveries WHERE commission_id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(Json(json!({"delivery": delivery})))
}

// ---------- 通知记录 ----------

pub async fn list_notifications(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    check_access(&user, &commission)?;
    let rows = sqlx::query_as::<_, Notification>(
        "SELECT * FROM notifications WHERE commission_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(rows)))
}
