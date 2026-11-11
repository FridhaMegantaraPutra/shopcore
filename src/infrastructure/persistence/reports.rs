use chrono::NaiveDate;
use sqlx::PgConnection;

use crate::domain::report::DailyReport;

pub async fn daily(conn: &mut PgConnection, date: NaiveDate) -> Result<DailyReport, sqlx::Error> {
    let (order_count, paid_cents, cancelled_cents, settled_cents): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*)::bigint,
                COALESCE(SUM(amount_cents) FILTER (WHERE status = 'paid'), 0)::bigint,
                COALESCE(SUM(amount_cents) FILTER (WHERE status = 'cancelled'), 0)::bigint,
                COALESCE(SUM(amount_cents) FILTER (WHERE status = 'settled'), 0)::bigint
         FROM orders
         WHERE created_at >= ($1::date)::timestamp AT TIME ZONE 'UTC'
           AND created_at <  ($1::date + 1)::timestamp AT TIME ZONE 'UTC'",
    )
    .bind(date)
    .fetch_one(conn)
    .await?;
    Ok(DailyReport { date, order_count, paid_cents, cancelled_cents, settled_cents })
}
