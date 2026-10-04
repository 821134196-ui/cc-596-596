use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

use crate::auth::hash_password;

pub async fn init(database_url: &str) -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5))
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    seed(&pool).await?;
    Ok(pool)
}

/// 预置演示账号：员工 staff/staff123，客户 chen/123456、liu/123456
async fn seed(pool: &SqlitePool) -> anyhow::Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(pool).await?;
    if count.0 > 0 {
        return Ok(());
    }
    let users = [
        ("staff", "staff123", "staff", "王店员", "021-55550001"),
        ("chen", "123456", "customer", "陈女士", "13800000001"),
        ("liu", "123456", "customer", "刘先生", "13800000002"),
    ];
    for (username, password, role, name, phone) in users {
        sqlx::query("INSERT INTO users (username, password, role, name, phone) VALUES (?,?,?,?,?)")
            .bind(username)
            .bind(hash_password(password))
            .bind(role)
            .bind(name)
            .bind(phone)
            .execute(pool)
            .await?;
    }
    println!("[seed] 已创建演示账号 staff/staff123, chen/123456, liu/123456");
    Ok(())
}
