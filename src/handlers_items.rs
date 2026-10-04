use axum::{
    extract::{Multipart, Path},
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::{AuthUser, StaffUser};
use crate::error::ApiError;
use crate::handlers_commission::{check_access, load_commission};
use crate::models::*;
use crate::AppState;

// ---------- 遗物 / 骨灰罐 ----------

pub async fn list_items(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    check_access(&user, &commission)?;
    let items = sqlx::query_as::<_, Item>(
        "SELECT * FROM items WHERE commission_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    // 附带保管人姓名
    let mut out = Vec::new();
    for item in items {
        let holder_name: Option<String> = match item.current_holder_id {
            Some(h) => sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
                .bind(h)
                .fetch_optional(&state.pool)
                .await?,
            None => None,
        };
        out.push(json!({"item": item, "holder_name": holder_name}));
    }
    Ok(Json(json!(out)))
}

pub async fn add_item(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(staff): StaffUser,
    Path(id): Path<i64>,
    Json(req): Json<AddItemReq>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    if commission.delivery_status == "delivered" {
        return Err(ApiError::conflict("该委托已完成交付，不能再登记物品"));
    }
    if !["relic", "urn"].contains(&req.item_type.as_str()) {
        return Err(ApiError::bad_request("物品类型必须为 relic（遗物）或 urn（骨灰罐）"));
    }
    if req.name.trim().is_empty() || req.seal_no.trim().is_empty() {
        return Err(ApiError::bad_request("物品名称与封签号不能为空"));
    }
    let item_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO items (commission_id, item_type, name, description, seal_no, current_holder_id)
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(id)
    .bind(&req.item_type)
    .bind(req.name.trim())
    .bind(req.description.clone().unwrap_or_default())
    .bind(req.seal_no.trim())
    .bind(staff.id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            ApiError::conflict(format!("封签号 {} 在该委托下已存在", req.seal_no))
        } else {
            ApiError::from(e)
        }
    })?;
    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = ?")
        .bind(item_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"item": item})))
}

// ---------- 照片（本地存储） ----------

pub async fn upload_photo(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(_staff): StaffUser,
    Path(item_id): Path<i64>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = ?")
        .bind(item_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("物品不存在"))?;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("上传解析失败: {e}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let content_type = field
            .content_type()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if !content_type.starts_with("image/") {
            return Err(ApiError::bad_request("仅支持图片文件"));
        }
        let ext = match content_type.as_str() {
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "jpg",
        };
        let data = field
            .bytes()
            .await
            .map_err(|e| ApiError::bad_request(format!("读取上传内容失败: {e}")))?;
        if data.len() > 10 * 1024 * 1024 {
            return Err(ApiError::bad_request("图片不能超过 10MB"));
        }
        let filename = format!("{}.{ext}", Uuid::new_v4());
        let path = state.photo_dir.join(&filename);
        tokio::fs::write(&path, &data)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query("UPDATE items SET photo_path = ? WHERE id = ?")
            .bind(&filename)
            .bind(item.id)
            .execute(&state.pool)
            .await?;
        return Ok(Json(json!({"photo_path": filename})));
    }
    Err(ApiError::bad_request("请求中未包含文件"))
}

// ---------- 移交与签收 ----------

/// 发起移交：当前保管人（或任意员工，当物品尚无保管人）把物品移交给另一名员工
pub async fn create_transfer(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(staff): StaffUser,
    Path(item_id): Path<i64>,
    Json(req): Json<CreateTransferReq>,
) -> Result<Json<Value>, ApiError> {
    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = ?")
        .bind(item_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("物品不存在"))?;
    if item.status == "delivered" {
        return Err(ApiError::conflict("物品已交付，不能再移交"));
    }
    if item.status == "in_transit" {
        return Err(ApiError::conflict("物品正处于移交中，等待接收人签收"));
    }
    if let Some(holder) = item.current_holder_id {
        if holder != staff.id {
            return Err(ApiError::forbidden("只有当前保管人才能发起移交"));
        }
    }
    let receiver = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, username, name FROM users WHERE id = ? AND role = 'staff'",
    )
    .bind(req.to_holder_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::bad_request("接收人必须是门店员工"))?;
    if receiver.0 == staff.id {
        return Err(ApiError::bad_request("不能移交给自己"));
    }

    let from_name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
        .bind(staff.id)
        .fetch_one(&state.pool)
        .await?;

    let transfer_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO transfers (commission_id, item_id, from_holder, to_holder_id, to_holder_name, transfer_type, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(item.commission_id)
    .bind(item.id)
    .bind(&from_name)
    .bind(receiver.0)
    .bind(&receiver.2)
    .bind(&req.transfer_type)
    .bind(req.notes.clone().unwrap_or_default())
    .fetch_one(&state.pool)
    .await?;
    sqlx::query("UPDATE items SET status = 'in_transit' WHERE id = ?")
        .bind(item.id)
        .execute(&state.pool)
        .await?;
    let transfer = sqlx::query_as::<_, Transfer>("SELECT * FROM transfers WHERE id = ?")
        .bind(transfer_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"transfer": transfer})))
}

/// 接收人签收：只有指定接收人本人可以签收
pub async fn sign_transfer(
    axum::extract::State(state): axum::extract::State<AppState>,
    StaffUser(staff): StaffUser,
    Path(transfer_id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let transfer = sqlx::query_as::<_, Transfer>("SELECT * FROM transfers WHERE id = ?")
        .bind(transfer_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("移交记录不存在"))?;
    if transfer.status == "signed" {
        return Err(ApiError::conflict("该移交已签收，不能重复签收"));
    }
    if transfer.to_holder_id != Some(staff.id) {
        return Err(ApiError::forbidden("只有指定接收人本人才能签收"));
    }
    sqlx::query(
        "UPDATE transfers SET status='signed', signed_by=?, signed_at=datetime('now','localtime') WHERE id=?",
    )
    .bind(staff.id)
    .bind(transfer.id)
    .execute(&state.pool)
    .await?;
    sqlx::query("UPDATE items SET status='in_store', current_holder_id=? WHERE id=?")
        .bind(staff.id)
        .bind(transfer.item_id)
        .execute(&state.pool)
        .await?;
    let transfer = sqlx::query_as::<_, Transfer>("SELECT * FROM transfers WHERE id = ?")
        .bind(transfer.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"transfer": transfer})))
}

pub async fn list_transfers(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let commission = load_commission(&state.pool, id).await?;
    check_access(&user, &commission)?;
    let transfers = sqlx::query_as::<_, Transfer>(
        "SELECT * FROM transfers WHERE commission_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    // 附带物品名称
    let mut out = Vec::new();
    for t in transfers {
        let item_name: String = sqlx::query_scalar("SELECT name FROM items WHERE id = ?")
            .bind(t.item_id)
            .fetch_one(&state.pool)
            .await?;
        out.push(json!({"transfer": t, "item_name": item_name}));
    }
    Ok(Json(json!(out)))
}
