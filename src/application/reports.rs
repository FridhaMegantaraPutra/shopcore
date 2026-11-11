use chrono::NaiveDate;
use sqlx::PgPool;

use super::AppResult;
use crate::domain::report::DailyReport;
use crate::infrastructure::persistence::reports as repo;

pub async fn daily_report(db: &PgPool, date: NaiveDate) -> AppResult<DailyReport> {
    Ok(repo::daily(&mut *db.acquire().await?, date).await?)
}
