use sqlx::PgPool;

use super::{AppError, AppResult};
use crate::domain::money::{Amount, Currency};
use crate::domain::wallet::{LedgerEntry, NewWallet, Owner, RecordedEntry, Wallet, WalletId};
use crate::infrastructure::persistence::wallets as repo;

pub async fn open_wallet(db: &PgPool, owner: &str, currency: Option<&str>) -> AppResult<Wallet> {
    let new = NewWallet { owner: Owner::parse(owner)?, currency: Currency::parse(currency)? };
    Ok(repo::insert(&mut *db.acquire().await?, &new).await?)
}

pub async fn get_wallet(db: &PgPool, id: WalletId) -> AppResult<Wallet> {
    repo::find(&mut *db.acquire().await?, id).await?.ok_or(AppError::NotFound("wallet"))
}

pub async fn top_up(db: &PgPool, id: WalletId, amount_cents: i64) -> AppResult<Wallet> {
    let amount = Amount::new(amount_cents)?;
    let mut tx = db.begin().await?;
    let mut wallet = repo::find_for_update(&mut tx, id).await?.ok_or(AppError::NotFound("wallet"))?;
    wallet.credit(amount);
    repo::save_balance(&mut tx, &wallet).await?;
    repo::append_entry(&mut tx, &LedgerEntry::topup(id, amount)).await?;
    tx.commit().await?;
    Ok(wallet)
}

pub async fn transactions(db: &PgPool, id: WalletId) -> AppResult<Vec<RecordedEntry>> {
    let mut conn = db.acquire().await?;
    repo::find(&mut conn, id).await?.ok_or(AppError::NotFound("wallet"))?;
    Ok(repo::entries(&mut conn, id).await?)
}
