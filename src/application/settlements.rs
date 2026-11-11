use sqlx::PgPool;

use super::{AppError, AppResult};
use crate::domain::settlement::{Settlement, SettlementOutcome};
use crate::infrastructure::persistence::{orders as order_repo, settlements as settlement_repo};

/// Applies a (signature-verified) bank settlement.
///
/// Idempotent: a redelivered `reference` returns `Duplicate` and changes
/// nothing. A rejected settlement (wrong state/amount) is rolled back, so the
/// reference is not consumed and the bank may retry with corrected data.
pub async fn apply_settlement(db: &PgPool, s: Settlement) -> AppResult<SettlementOutcome> {
    let s = s.validate()?;
    let mut tx = db.begin().await?;
    let mut order = order_repo::find_for_update(&mut tx, s.order_id).await?.ok_or(AppError::NotFound("order"))?;
    if !settlement_repo::try_record(&mut tx, &s).await? {
        return Ok(SettlementOutcome::Duplicate);
    }
    order.settle(s.amount_cents)?;
    order_repo::save_status(&mut tx, &order).await?;
    tx.commit().await?;
    Ok(SettlementOutcome::Settled)
}
