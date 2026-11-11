use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::domain::money::Amount;
use crate::domain::order::{Order, OrderId, OrderStatus};
use crate::domain::wallet::WalletId;

#[derive(sqlx::FromRow)]
struct OrderRow {
    id: i64,
    wallet_id: i64,
    amount_cents: i64,
    status: String,
    created_at: DateTime<Utc>,
}

impl TryFrom<OrderRow> for Order {
    type Error = sqlx::Error;
    fn try_from(r: OrderRow) -> Result<Self, Self::Error> {
        Ok(Order {
            id: r.id,
            wallet_id: r.wallet_id,
            amount: Amount::new(r.amount_cents).map_err(super::corrupt)?,
            status: OrderStatus::parse(&r.status).map_err(super::corrupt)?,
            created_at: r.created_at,
        })
    }
}

const COLS: &str = "id, wallet_id, amount_cents, status, created_at";

pub async fn insert(conn: &mut PgConnection, wallet_id: WalletId, amount: Amount) -> Result<Order, sqlx::Error> {
    let row: OrderRow = sqlx::query_as(&format!(
        "INSERT INTO orders (wallet_id, amount_cents, status) VALUES ($1, $2, 'paid') RETURNING {COLS}"
    ))
    .bind(wallet_id)
    .bind(amount.cents())
    .fetch_one(conn)
    .await?;
    row.try_into()
}

pub async fn find(conn: &mut PgConnection, id: OrderId) -> Result<Option<Order>, sqlx::Error> {
    let row: Option<OrderRow> = sqlx::query_as(&format!("SELECT {COLS} FROM orders WHERE id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await?;
    row.map(TryInto::try_into).transpose()
}

pub async fn find_for_update(conn: &mut PgConnection, id: OrderId) -> Result<Option<Order>, sqlx::Error> {
    let row: Option<OrderRow> = sqlx::query_as(&format!("SELECT {COLS} FROM orders WHERE id = $1 FOR UPDATE"))
        .bind(id)
        .fetch_optional(conn)
        .await?;
    row.map(TryInto::try_into).transpose()
}

pub async fn save_status(conn: &mut PgConnection, o: &Order) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE orders SET status = $2 WHERE id = $1")
        .bind(o.id)
        .bind(o.status.as_str())
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn list(
    conn: &mut PgConnection,
    wallet_id: Option<WalletId>,
    status: Option<OrderStatus>,
) -> Result<Vec<Order>, sqlx::Error> {
    let rows: Vec<OrderRow> = sqlx::query_as(&format!(
        "SELECT {COLS} FROM orders
         WHERE ($1::bigint IS NULL OR wallet_id = $1)
           AND ($2::text IS NULL OR status = $2)
         ORDER BY id"
    ))
    .bind(wallet_id)
    .bind(status.map(OrderStatus::as_str))
    .fetch_all(conn)
    .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}
