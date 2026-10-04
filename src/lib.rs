pub mod auth;
pub mod error;
pub mod handlers_commission;
pub mod handlers_delivery;
pub mod handlers_items;
pub mod models;
pub mod notify;
pub mod seed;

use std::path::PathBuf;
use std::time::Duration;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub photo_dir: PathBuf,
}

pub async fn init_pool(db_path: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub fn build_router(state: AppState, dist_dir: PathBuf) -> Router {
    let api = Router::new()
        // 认证
        .route("/auth/login", post(handlers_commission::login))
        .route("/auth/me", get(handlers_commission::me))
        .route("/auth/logout", post(handlers_commission::logout))
        // 员工
        .route("/staff", get(handlers_commission::list_staff))
        // 委托单
        .route(
            "/commissions",
            get(handlers_commission::list_commissions).post(handlers_commission::create_commission),
        )
        .route("/commissions/{id}", get(handlers_commission::get_commission))
        .route("/commissions/{id}/pickup", put(handlers_commission::update_pickup))
        .route("/commissions/{id}/status", post(handlers_commission::update_status))
        // 服务项目与确认版本
        .route(
            "/commissions/{id}/service-items",
            get(handlers_commission::list_service_items).post(handlers_commission::add_service_item),
        )
        .route("/service-items/{id}", delete(handlers_commission::delete_service_item))
        .route(
            "/commissions/{id}/service-list/confirm",
            post(handlers_commission::confirm_service_list),
        )
        .route(
            "/commissions/{id}/service-list/versions",
            get(handlers_commission::list_service_versions),
        )
        // 遗物 / 骨灰罐 / 照片
        .route(
            "/commissions/{id}/items",
            get(handlers_items::list_items).post(handlers_items::add_item),
        )
        .route("/items/{id}/photo", post(handlers_items::upload_photo))
        // 移交
        .route("/items/{id}/transfers", post(handlers_items::create_transfer))
        .route("/transfers/{id}/sign", post(handlers_items::sign_transfer))
        .route("/commissions/{id}/transfers", get(handlers_items::list_transfers))
        // 代领授权
        .route(
            "/commissions/{id}/proxy",
            get(handlers_delivery::list_proxy).post(handlers_delivery::create_proxy),
        )
        .route("/proxy/{id}/revoke", post(handlers_delivery::revoke_proxy))
        // 出库与交付
        .route(
            "/commissions/{id}/outbound-requests",
            get(handlers_delivery::list_outbound).post(handlers_delivery::create_outbound),
        )
        .route("/commissions/{id}/deliver", post(handlers_delivery::deliver))
        .route("/commissions/{id}/delivery", get(handlers_delivery::get_delivery))
        // 通知
        .route("/commissions/{id}/notifications", get(handlers_delivery::list_notifications))
        .with_state(state.clone());

    let index_file = dist_dir.join("index.html");
    Router::new()
        .nest("/api", api)
        .nest_service("/photos", ServeDir::new(state.photo_dir.clone()))
        .fallback_service(ServeDir::new(dist_dir).fallback(ServeFile::new(index_file)))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
