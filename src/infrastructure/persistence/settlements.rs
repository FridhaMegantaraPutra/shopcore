use sqlx::PgConnection;

use crate::domain::settlement::Settlement;

/// Records the settlement. Returns `false` if this reference was already
/// recorded (the UNIQUE constraint makes this race-free).
pub async fn try_record(conn: &mut PgConnection, s: &Settlement) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "INSERT INTO settlements (reference, order_id, amount_cents) VALUES ($1, $2, $3)
         ON CONFLICT (reference) DO NOTHING",
    )
    .bind(&s.reference)
    .bind(s.order_id)
    .bind(s.amount_cents)
    .execute(conn)
    .await?;
    Ok(r.rows_affected() == 1)
}
