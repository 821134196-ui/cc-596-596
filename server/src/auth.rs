use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::models::User;
use crate::AppState;

pub fn hash_password(password: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"petmemorial-v1:");
    h.update(password.as_bytes());
    format!("{:x}", h.finalize())
}

/// 从 Authorization: Bearer <token> 解析当前用户
pub struct AuthUser(pub User);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("未登录"))?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::unauthorized("无效的登录凭证"))?;
        let user = sqlx::query_as::<_, User>(
            "SELECT u.id, u.username, u.password, u.role, u.name, u.phone \
             FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token = ?",
        )
        .bind(token)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::unauthorized("登录已失效，请重新登录"))?;
        Ok(AuthUser(user))
    }
}

impl AuthUser {
    pub fn require_staff(&self) -> Result<(), ApiError> {
        if self.0.role != "staff" {
            return Err(ApiError::forbidden("仅门店员工可执行此操作"));
        }
        Ok(())
    }
}
