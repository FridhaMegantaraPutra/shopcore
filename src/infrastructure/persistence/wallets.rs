use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::domain::wallet::{EntryKind, LedgerEntry, NewWallet, RecordedEntry, Wallet, WalletId};
use crate::domain::DomainError;

#[derive(sqlx::FromRow)]
struct WalletRow {
    id: i64,
    owner: String,
    currency: String,
    balance_cents: i64,
    created_at: DateTime<Utc>,
}

impl From<WalletRow> for Wallet {
    fn from(r: WalletRow) -> Self {
        Wallet { id: r.id, owner: r.owner, currency: r.currency, balance_cents: r.balance_cents, created_at: r.created_at }
    }
}

const COLS: &str = "id, owner, currency::text AS currency, balance_cents, created_at";

pub async fn insert(conn: &mut PgConnection, w: &NewWallet) -> Result<Wallet, sqlx::Error> {
    let row: WalletRow = sqlx::query_as(&format!("INSERT INTO wallets (owner, currency) VALUES ($1, $2) RETURNING {COLS}"))
        .bind(w.owner.as_str())
        .bind(w.currency.as_str())
        .fetch_one(conn)
        .await?;
    Ok(row.into())
}

pub async fn find(conn: &mut PgConnection, id: WalletId) -> Result<Option<Wallet>, sqlx::Error> {
    let row: Option<WalletRow> = sqlx::query_as(&format!("SELECT {COLS} FROM wallets WHERE id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await?;
    Ok(row.map(Into::into))
}

/// Loads the wallet and row-locks it until the transaction ends, so
/// concurrent debits are serialised.
pub async fn find_for_update(conn: &mut PgConnection, id: WalletId) -> Result<Option<Wallet>, sqlx::Error> {
    let row: Option<WalletRow> = sqlx::query_as(&format!("SELECT {COLS} FROM wallets WHERE id = $1 FOR UPDATE"))
        .bind(id)
        .fetch_optional(conn)
        .await?;
    Ok(row.map(Into::into))
}

pub async fn save_balance(conn: &mut PgConnection, w: &Wallet) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE wallets SET balance_cents = $2 WHERE id = $1")
        .bind(w.id)
        .bind(w.balance_cents)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn append_entry(conn: &mut PgConnection, e: &LedgerEntry) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO wallet_transactions (wallet_id, amount_cents, kind, order_id) VALUES ($1, $2, $3, $4)")
        .bind(e.wallet_id)
        .bind(e.amount_cents)
        .bind(e.kind.as_str())
        .bind(e.order_id)
        .execute(conn)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct EntryRow {
    id: i64,
    wallet_id: i64,
    amount_cents: i64,
    kind: String,
    order_id: Option<i64>,
    created_at: DateTime<Utc>,
}

pub async fn entries(conn: &mut PgConnection, wallet_id: WalletId) -> Result<Vec<RecordedEntry>, sqlx::Error> {
    let rows: Vec<EntryRow> = sqlx::query_as(
        "SELECT id, wallet_id, amount_cents, kind, order_id, created_at
         FROM wallet_transactions WHERE wallet_id = $1 ORDER BY id",
    )
    .bind(wallet_id)
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|r| {
            let kind = match r.kind.as_str() {
                "topup" => EntryKind::Topup,
                "order" => EntryKind::Order,
                "refund" => EntryKind::Refund,
                other => return Err(super::corrupt(DomainError::Validation(format!("unknown entry kind {other}")))),
            };
            Ok(RecordedEntry {
                id: r.id,
                entry: LedgerEntry { wallet_id: r.wallet_id, amount_cents: r.amount_cents, kind, order_id: r.order_id },
                created_at: r.created_at,
            })
        })
        .collect()
}

/// Sets every wallet balance to the sum of its ledger. Returns how many
/// wallets were out of sync.
pub async fn recompute_balances(conn: &mut PgConnection) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE wallets w SET balance_cents = t.total
         FROM (SELECT w2.id, COALESCE(SUM(tx.amount_cents), 0)::bigint AS total
               FROM wallets w2 LEFT JOIN wallet_transactions tx ON tx.wallet_id = w2.id
               GROUP BY w2.id) t
         WHERE w.id = t.id AND w.balance_cents <> t.total",
    )
    .execute(conn)
    .await?;
    Ok(r.rows_affected())
}
