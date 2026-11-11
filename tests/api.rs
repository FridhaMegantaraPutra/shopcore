//! End-to-end HTTP tests against a real Postgres.
//!
//! Run with: `DATABASE_URL=postgres://... cargo test`
//! Without DATABASE_URL these tests are skipped (they print a notice and pass),
//! so `cargo test` still works on a machine without a database.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use shopcore::application::maintenance;
use shopcore::infrastructure::{db, signature};
use shopcore::interfaces::http::{router, AppState};
use tower::ServiceExt;

const SECRET: &str = "test-secret";

async fn app() -> Option<Router> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set, skipping integration test");
        return None;
    };
    let pool = db::connect_with_retry(&url, 5).await.expect("connect");
    maintenance::migrate(&pool).await.expect("migrate");
    Some(router(AppState { db: pool, webhook_secret: SECRET.into() }))
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    send(app, req.body(body).unwrap()).await
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn settle(app: &Router, body: &Value, sig: Option<String>) -> (StatusCode, Value) {
    let raw = body.to_string();
    let mut req = Request::builder()
        .method("POST")
        .uri("/webhooks/bank/settlement")
        .header("content-type", "application/json");
    if let Some(sig) = sig {
        req = req.header(signature::HEADER, sig);
    }
    send(app, req.body(Body::from(raw)).unwrap()).await
}

async fn funded_wallet(app: &Router, cents: i64) -> i64 {
    let (s, w) = call(app, "POST", "/wallets", Some(json!({ "owner": "tester" }))).await;
    assert_eq!(s, StatusCode::CREATED);
    let id = w["id"].as_i64().unwrap();
    let (s, _) = call(app, "POST", &format!("/wallets/{id}/topup"), Some(json!({ "amount_cents": cents }))).await;
    assert_eq!(s, StatusCode::OK);
    id
}

#[tokio::test]
async fn health() {
    let Some(app) = app().await else { return };
    let (s, b) = call(&app, "GET", "/health", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["status"], "ok");
}

#[tokio::test]
async fn wallet_lifecycle_and_validation() {
    let Some(app) = app().await else { return };
    let id = funded_wallet(&app, 1_000).await;
    let (s, w) = call(&app, "GET", &format!("/wallets/{id}"), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(w["balance_cents"], 1_000);
    assert_eq!(w["currency"], "IDR");

    let (s, _) = call(&app, "POST", "/wallets", Some(json!({ "owner": "  " }))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(&app, "POST", &format!("/wallets/{id}/topup"), Some(json!({ "amount_cents": 0 }))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(&app, "GET", "/wallets/999999999", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn order_debits_and_cancel_refunds() {
    let Some(app) = app().await else { return };
    let wid = funded_wallet(&app, 1_000).await;

    let (s, o) = call(&app, "POST", "/orders", Some(json!({ "wallet_id": wid, "amount_cents": 400 }))).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(o["status"], "paid");
    let oid = o["id"].as_i64().unwrap();
    let (_, w) = call(&app, "GET", &format!("/wallets/{wid}"), None).await;
    assert_eq!(w["balance_cents"], 600);

    let (s, _) = call(&app, "POST", "/orders", Some(json!({ "wallet_id": wid, "amount_cents": 601 }))).await;
    assert_eq!(s, StatusCode::PAYMENT_REQUIRED);

    let (s, o) = call(&app, "POST", &format!("/orders/{oid}/cancel"), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(o["status"], "cancelled");
    let (_, w) = call(&app, "GET", &format!("/wallets/{wid}"), None).await;
    assert_eq!(w["balance_cents"], 1_000);

    let (s, _) = call(&app, "POST", &format!("/orders/{oid}/cancel"), None).await;
    assert_eq!(s, StatusCode::CONFLICT, "double cancel must not refund twice");

    let (_, txs) = call(&app, "GET", &format!("/wallets/{wid}/transactions"), None).await;
    let kinds: Vec<&str> = txs.as_array().unwrap().iter().map(|t| t["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["topup", "order", "refund"]);
}

#[tokio::test]
async fn concurrent_orders_cannot_overdraw() {
    let Some(app) = app().await else { return };
    let wid = funded_wallet(&app, 1_000).await;
    let tasks: Vec<_> = (0..10)
        .map(|_| {
            let app = app.clone();
            tokio::spawn(async move {
                call(&app, "POST", "/orders", Some(json!({ "wallet_id": wid, "amount_cents": 300 }))).await.0
            })
        })
        .collect();
    let mut created = 0;
    for t in tasks {
        if t.await.unwrap() == StatusCode::CREATED {
            created += 1;
        }
    }
    assert_eq!(created, 3);
    let (_, w) = call(&app, "GET", &format!("/wallets/{wid}"), None).await;
    assert_eq!(w["balance_cents"], 100);
}

#[tokio::test]
async fn settlement_webhook() {
    let Some(app) = app().await else { return };
    let wid = funded_wallet(&app, 1_000).await;
    let (_, o) = call(&app, "POST", "/orders", Some(json!({ "wallet_id": wid, "amount_cents": 250 }))).await;
    let oid = o["id"].as_i64().unwrap();
    let reference = format!("TEST-{oid}-{}", std::process::id());
    let body = json!({ "reference": reference, "order_id": oid, "amount_cents": 250 });

    let (s, _) = settle(&app, &body, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = settle(&app, &body, Some(signature::sign("wrong", body.to_string().as_bytes()))).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let wrong_amount = json!({ "reference": format!("{reference}-x"), "order_id": oid, "amount_cents": 1 });
    let (s, _) = settle(&app, &wrong_amount, Some(signature::sign(SECRET, wrong_amount.to_string().as_bytes()))).await;
    assert_eq!(s, StatusCode::CONFLICT);

    let sig = signature::sign(SECRET, body.to_string().as_bytes());
    let (s, r) = settle(&app, &body, Some(sig.clone())).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(r["status"], "settled");
    let (s, r) = settle(&app, &body, Some(sig)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(r["status"], "duplicate");

    let (_, o) = call(&app, "GET", &format!("/orders/{oid}"), None).await;
    assert_eq!(o["status"], "settled");
    let (s, _) = call(&app, "POST", &format!("/orders/{oid}/cancel"), None).await;
    assert_eq!(s, StatusCode::CONFLICT, "settled orders cannot be cancelled");
}

#[tokio::test]
async fn daily_report_shape_and_validation() {
    let Some(app) = app().await else { return };
    let (s, r) = call(&app, "GET", "/reports/daily?date=2000-01-01", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(r, json!({ "date": "2000-01-01", "order_count": 0, "paid_cents": 0, "cancelled_cents": 0, "settled_cents": 0 }));
    let (s, _) = call(&app, "GET", "/reports/daily?date=yesterday", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}
