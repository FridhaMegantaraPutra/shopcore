//! Thin HTTP handlers: parse request → call use case → map to DTO.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::{NaiveDate, Utc};
use serde_json::{json, Value};

use super::dto::*;
use super::error::{HttpError, HttpResult};
use super::AppState;
use crate::application::{orders, reports, settlements, wallets, AppError};
use crate::domain::report::DailyReport;
use crate::domain::settlement::{Settlement, SettlementOutcome};
use crate::infrastructure::signature;

pub async fn health(State(st): State<AppState>) -> HttpResult<Json<Value>> {
    sqlx::query("SELECT 1").execute(&st.db).await.map_err(AppError::from)?;
    Ok(Json(json!({ "status": "ok" })))
}

pub async fn create_wallet(
    State(st): State<AppState>,
    Json(req): Json<CreateWalletRequest>,
) -> HttpResult<(StatusCode, Json<WalletResponse>)> {
    let w = wallets::open_wallet(&st.db, &req.owner, req.currency.as_deref()).await?;
    Ok((StatusCode::CREATED, Json(w.into())))
}

pub async fn get_wallet(State(st): State<AppState>, Path(id): Path<i64>) -> HttpResult<Json<WalletResponse>> {
    Ok(Json(wallets::get_wallet(&st.db, id).await?.into()))
}

pub async fn topup_wallet(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<AmountRequest>,
) -> HttpResult<Json<WalletResponse>> {
    Ok(Json(wallets::top_up(&st.db, id, req.amount_cents).await?.into()))
}

pub async fn list_transactions(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> HttpResult<Json<Vec<TransactionResponse>>> {
    let rows = wallets::transactions(&st.db, id).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

pub async fn create_order(
    State(st): State<AppState>,
    Json(req): Json<CreateOrderRequest>,
) -> HttpResult<(StatusCode, Json<OrderResponse>)> {
    let o = orders::place_order(&st.db, req.wallet_id, req.amount_cents).await?;
    Ok((StatusCode::CREATED, Json(o.into())))
}

pub async fn get_order(State(st): State<AppState>, Path(id): Path<i64>) -> HttpResult<Json<OrderResponse>> {
    Ok(Json(orders::get_order(&st.db, id).await?.into()))
}

pub async fn list_orders(
    State(st): State<AppState>,
    Query(f): Query<OrderFilter>,
) -> HttpResult<Json<Vec<OrderResponse>>> {
    let rows = orders::list_orders(&st.db, f.wallet_id, f.status.as_deref()).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

pub async fn cancel_order(State(st): State<AppState>, Path(id): Path<i64>) -> HttpResult<Json<OrderResponse>> {
    Ok(Json(orders::cancel_order(&st.db, id).await?.into()))
}

pub async fn daily_report(State(st): State<AppState>, Query(q): Query<ReportQuery>) -> HttpResult<Json<DailyReport>> {
    let date = match q.date {
        Some(d) => NaiveDate::parse_from_str(&d, "%Y-%m-%d")
            .map_err(|_| HttpError::BadRequest("date must be YYYY-MM-DD".into()))?,
        None => Utc::now().date_naive(),
    };
    Ok(Json(reports::daily_report(&st.db, date).await?))
}

/// Bank settlement webhook. The signature is checked over the *raw* body
/// before anything is parsed.
pub async fn bank_settlement(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> HttpResult<Json<Value>> {
    let sig = headers
        .get(signature::HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(HttpError::Unauthorized("missing signature"))?;
    if !signature::verify(&st.webhook_secret, &body, sig) {
        return Err(HttpError::Unauthorized("invalid signature"));
    }
    let s: Settlement =
        serde_json::from_slice(&body).map_err(|e| HttpError::BadRequest(format!("invalid payload: {e}")))?;
    let (reference, order_id) = (s.reference.clone(), s.order_id);
    let status = match settlements::apply_settlement(&st.db, s).await? {
        SettlementOutcome::Settled => "settled",
        SettlementOutcome::Duplicate => "duplicate",
    };
    Ok(Json(json!({ "status": status, "reference": reference, "order_id": order_id })))
}
