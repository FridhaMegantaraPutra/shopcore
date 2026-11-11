mod dto;
mod error;
mod handlers;

use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;

/// Shared state handed to every HTTP handler.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Shared secret used to verify the bank's HMAC-SHA256 webhook signature.
    pub webhook_secret: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/wallets", post(handlers::create_wallet))
        .route("/wallets/{id}", get(handlers::get_wallet))
        .route("/wallets/{id}/topup", post(handlers::topup_wallet))
        .route("/wallets/{id}/transactions", get(handlers::list_transactions))
        .route("/orders", post(handlers::create_order).get(handlers::list_orders))
        .route("/orders/{id}", get(handlers::get_order))
        .route("/orders/{id}/cancel", post(handlers::cancel_order))
        .route("/reports/daily", get(handlers::daily_report))
        .route("/webhooks/bank/settlement", post(handlers::bank_settlement))
        .with_state(state)
}
