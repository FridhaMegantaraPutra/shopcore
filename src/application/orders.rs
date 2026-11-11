use sqlx::PgPool;

use super::{AppError, AppResult};
use crate::domain::money::Amount;
use crate::domain::order::{Order, OrderId, OrderStatus};
use crate::domain::wallet::{LedgerEntry, WalletId};
use crate::infrastructure::persistence::{orders as order_repo, wallets as wallet_repo};

/// Places an order paid from the wallet. Wallet debit, order creation and the
/// ledger entry are committed atomically; the wallet row lock prevents
/// concurrent orders from overdrawing it.
pub async fn place_order(db: &PgPool, wallet_id: WalletId, amount_cents: i64) -> AppResult<Order> {
    let amount = Amount::new(amount_cents)?;
    let mut tx = db.begin().await?;
    let mut wallet = wallet_repo::find_for_update(&mut tx, wallet_id).await?.ok_or(AppError::NotFound("wallet"))?;
    wallet.debit(amount)?;
    wallet_repo::save_balance(&mut tx, &wallet).await?;
    let order = order_repo::insert(&mut tx, wallet_id, amount).await?;
    wallet_repo::append_entry(&mut tx, &LedgerEntry::order_payment(wallet_id, amount, order.id)).await?;
    tx.commit().await?;
    Ok(order)
}

pub async fn get_order(db: &PgPool, id: OrderId) -> AppResult<Order> {
    order_repo::find(&mut *db.acquire().await?, id).await?.ok_or(AppError::NotFound("order"))
}

pub async fn list_orders(db: &PgPool, wallet_id: Option<WalletId>, status: Option<&str>) -> AppResult<Vec<Order>> {
    let status = status.map(OrderStatus::parse).transpose()?;
    Ok(order_repo::list(&mut *db.acquire().await?, wallet_id, status).await?)
}

/// Cancels a paid order and refunds its wallet.
pub async fn cancel_order(db: &PgPool, id: OrderId) -> AppResult<Order> {
    let mut tx = db.begin().await?;
    let mut order = order_repo::find_for_update(&mut tx, id).await?.ok_or(AppError::NotFound("order"))?;
    order.cancel()?;
    // Lock order, then wallet. No deadlock with place_order (which locks the
    // wallet and only inserts a *new* order row) or top_up (wallet only).
    let mut wallet = wallet_repo::find_for_update(&mut tx, order.wallet_id)
        .await?
        .ok_or(AppError::NotFound("wallet"))?;
    wallet.credit(order.amount);
    order_repo::save_status(&mut tx, &order).await?;
    wallet_repo::save_balance(&mut tx, &wallet).await?;
    wallet_repo::append_entry(&mut tx, &LedgerEntry::refund(wallet.id, order.amount, order.id)).await?;
    tx.commit().await?;
    Ok(order)
}
