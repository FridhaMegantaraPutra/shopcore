//! Request/response shapes of the public JSON API. Kept separate from domain
//! types so the wire format can't change by accident when the domain does.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::order::Order;
use crate::domain::wallet::{RecordedEntry, Wallet};

#[derive(Deserialize)]
pub struct CreateWalletRequest {
    pub owner: String,
    pub currency: Option<String>,
}

#[derive(Deserialize)]
pub struct AmountRequest {
    pub amount_cents: i64,
}

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub wallet_id: i64,
    pub amount_cents: i64,
}

#[derive(Deserialize)]
pub struct OrderFilter {
    pub wallet_id: Option<i64>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct ReportQuery {
    pub date: Option<String>,
}

#[derive(Serialize)]
pub struct WalletResponse {
    pub id: i64,
    pub owner: String,
    pub currency: String,
    pub balance_cents: i64,
    pub created_at: DateTime<Utc>,
}

impl From<Wallet> for WalletResponse {
    fn from(w: Wallet) -> Self {
        Self { id: w.id, owner: w.owner, currency: w.currency, balance_cents: w.balance_cents, created_at: w.created_at }
    }
}

#[derive(Serialize)]
pub struct OrderResponse {
    pub id: i64,
    pub wallet_id: i64,
    pub amount_cents: i64,
    pub status: &'static str,
    pub created_at: DateTime<Utc>,
}

impl From<Order> for OrderResponse {
    fn from(o: Order) -> Self {
        Self {
            id: o.id,
            wallet_id: o.wallet_id,
            amount_cents: o.amount.cents(),
            status: o.status.as_str(),
            created_at: o.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub id: i64,
    pub wallet_id: i64,
    pub amount_cents: i64,
    pub kind: &'static str,
    pub order_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl From<RecordedEntry> for TransactionResponse {
    fn from(r: RecordedEntry) -> Self {
        Self {
            id: r.id,
            wallet_id: r.entry.wallet_id,
            amount_cents: r.entry.amount_cents,
            kind: r.entry.kind.as_str(),
            order_id: r.entry.order_id,
            created_at: r.created_at,
        }
    }
}
