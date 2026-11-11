//! Maintenance use cases behind the CLI (equivalent of legacy `manage.py`).

use sqlx::PgPool;

use super::AppResult;
use crate::domain::money::Amount;
use crate::domain::wallet::{LedgerEntry, NewWallet, Owner};
use crate::domain::money::Currency;
use crate::infrastructure::db::MIGRATOR;
use crate::infrastructure::persistence::wallets as repo;

pub async fn migrate(db: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(db).await
}

pub async fn recompute_balances(db: &PgPool) -> AppResult<u64> {
    let mut tx = db.begin().await?;
    let n = repo::recompute_balances(&mut tx).await?;
    tx.commit().await?;
    Ok(n)
}

/// Creates two demo wallets with an opening top-up each.
pub async fn seed(db: &PgPool) -> AppResult<()> {
    let mut tx = db.begin().await?;
    for (owner, cents) in [("alice", 100_000), ("bob", 50_000)] {
        let mut w = repo::insert(&mut tx, &NewWallet { owner: Owner::parse(owner)?, currency: Currency::parse(None)? }).await?;
        let amount = Amount::new(cents)?;
        w.credit(amount);
        repo::save_balance(&mut tx, &w).await?;
        repo::append_entry(&mut tx, &LedgerEntry::topup(w.id, amount)).await?;
    }
    tx.commit().await?;
    Ok(())
}
