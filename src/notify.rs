use sqlx::SqlitePool;

/// 模拟通知接口：不真正发送短信，只落库并打印日志
pub async fn send_mock(
    pool: &SqlitePool,
    commission_id: Option<i64>,
    recipient: &str,
    content: &str,
) {
    let res = sqlx::query(
        "INSERT INTO notifications (commission_id, recipient, channel, content, status)
         VALUES (?, ?, 'sms', ?, 'mock_sent')",
    )
    .bind(commission_id)
    .bind(recipient)
    .bind(content)
    .execute(pool)
    .await;
    match res {
        Ok(_) => tracing::info!("[模拟通知] 收件人={} 内容={}", recipient, content),
        Err(e) => tracing::error!("[模拟通知] 写入失败: {e}"),
    }
}
