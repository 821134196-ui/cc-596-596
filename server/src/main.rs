mod auth;
mod db;
mod error;
mod handlers;
mod models;

use axum::{
    routing::{get, post, put},
    Router,
};
use sqlx::SqlitePool;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub upload_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/app.db".into());
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "uploads".into());
    let web_dist = std::env::var("WEB_DIST").unwrap_or_else(|_| "../web/dist".into());
    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());

    if let Some(parent) = database_url.strip_prefix("sqlite:").and_then(|p| std::path::Path::new(p).parent()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir_all(&upload_dir)?;

    let pool = db::init(&database_url).await?;
    let state = AppState { pool, upload_dir: upload_dir.clone() };

    let api = Router::new()
        // 认证
        .route("/login", post(handlers::login))
        .route("/me", get(handlers::me))
        .route("/logout", post(handlers::logout))
        // 员工端
        .route("/staff/commissions", get(handlers::list_commissions).post(handlers::create_commission))
        .route("/staff/commissions/{id}", get(handlers::staff_commission_detail))
        .route("/staff/commissions/{id}/pickup", put(handlers::update_pickup))
        .route("/staff/commissions/{id}/items", put(handlers::set_service_items))
        .route("/staff/commissions/{id}/confirm", post(handlers::confirm_service_list))
        .route("/staff/commissions/{id}/relics", post(handlers::add_relic))
        .route("/staff/commissions/{cid}/relics/{rid}/photo", post(handlers::upload_relic_photo))
        .route("/staff/commissions/{id}/seals", post(handlers::add_seal))
        .route("/staff/commissions/{id}/transfers", post(handlers::add_transfer))
        .route("/staff/commissions/{id}/proxies", post(handlers::add_proxy))
        .route("/staff/commissions/{cid}/proxies/{pid}/revoke", post(handlers::revoke_proxy))
        .route("/staff/commissions/{id}/complete-service", post(handlers::complete_service))
        .route("/staff/delivery-info/{code}", get(handlers::delivery_info))
        // 交付出库
        .route("/deliveries", post(handlers::deliver))
        // 客户端（仅本人）
        .route("/my/commissions", get(handlers::my_commissions))
        .route("/my/commissions/{id}", get(handlers::my_commission_detail))
        .with_state(state.clone());

    let index = format!("{web_dist}/index.html");
    let app = Router::new()
        .nest("/api", api)
        .nest_service("/uploads", ServeDir::new(&upload_dir))
        .fallback_service(ServeDir::new(&web_dist).fallback(ServeFile::new(index)));

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("宠物殡葬遗物封存与骨灰交付系统已启动: http://localhost:{bind}");
    println!("  员工账号 staff/staff123 · 客户账号 chen/123456, liu/123456");
    axum::serve(listener, app).await?;
    Ok(())
}
