use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String, // staff | customer
    pub name: String,
    pub phone: String,
    pub id_no: String,
}

impl User {
    pub fn is_staff(&self) -> bool {
        self.role == "staff"
    }
}

pub fn hash_password(password: &str) -> String {
    // 演示项目使用 盐+SHA256；生产环境应使用 argon2/bcrypt
    let mut h = Sha256::new();
    h.update(b"pet-memorial-salt:");
    h.update(password.as_bytes());
    hex::encode(h.finalize())
}

pub async fn create_session(pool: &SqlitePool, user_id: i64) -> Result<String, ApiError> {
    let token = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions (token, user_id) VALUES (?, ?)")
        .bind(&token)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(token)
}

pub async fn find_user_by_token(pool: &SqlitePool, token: &str) -> Result<Option<User>, ApiError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT u.id, u.username, u.password_hash, u.role, u.name, u.phone, u.id_no
         FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token = ?",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

/// 登录用户提取器：从 Authorization: Bearer <token> 解析
pub struct AuthUser(pub User);

impl FromRequestParts<crate::AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        let header_value = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized("未登录：缺少 Authorization 头".into()))?;
        let token = header_value
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::Unauthorized("未登录：Authorization 格式错误".into()))?;
        let user = find_user_by_token(&state.pool, token)
            .await?
            .ok_or_else(|| ApiError::Unauthorized("登录已失效，请重新登录".into()))?;
        Ok(AuthUser(user))
    }
}

/// 仅员工可用
pub struct StaffUser(pub User);

impl FromRequestParts<crate::AppState> for StaffUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_staff() {
            return Err(ApiError::forbidden("仅门店员工可执行此操作"));
        }
        Ok(StaffUser(user))
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResp {
    pub token: String,
    pub user: User,
}
