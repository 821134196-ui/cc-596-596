use std::path::PathBuf;

use pet_memorial::{build_router, init_pool, seed, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pet_memorial=info,tower_http=info".into()),
        )
        .init();

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "data/app.db".into());
    let photo_dir = PathBuf::from(std::env::var("PHOTO_DIR").unwrap_or_else(|_| "photos".into()));
    let dist_dir =
        PathBuf::from(std::env::var("DIST_DIR").unwrap_or_else(|_| "frontend/dist".into()));
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    if let Some(parent) = PathBuf::from(&db_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::create_dir_all(&photo_dir).await?;

    let pool = init_pool(&db_path).await?;
    seed::seed(&pool).await?;
    tracing::info!("数据库就绪: {db_path}");

    let state = AppState { pool, photo_dir };
    let app = build_router(state, dist_dir);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("服务已启动: http://localhost:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
