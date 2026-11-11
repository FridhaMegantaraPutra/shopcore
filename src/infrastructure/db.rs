use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// SQL migrations, embedded into the binary at compile time.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Connects with retries so the service survives Postgres starting slower
/// than the app container.
pub async fn connect_with_retry(database_url: &str, attempts: u32) -> Result<PgPool, sqlx::Error> {
    let mut last = None;
    for i in 1..=attempts.max(1) {
        match PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await
        {
            Ok(pool) => return Ok(pool),
            Err(e) => {
                tracing::warn!(attempt = i, error = %e, "database not ready, retrying");
                last = Some(e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    Err(last.expect("at least one attempt"))
}
