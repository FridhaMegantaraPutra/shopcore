//! Minimal bank simulator: polls for `paid` orders and posts a signed
//! settlement webhook for each one. Stand-in for the legacy bank simulator.

use std::time::Duration;

use shopcore::infrastructure::signature;

#[tokio::main]
async fn main() {
    let api = std::env::var("SHOPCORE_URL").unwrap_or_else(|_| "http://app:8000".into());
    let secret = std::env::var("BANK_WEBHOOK_SECRET").expect("BANK_WEBHOOK_SECRET must be set");
    let interval: u64 = std::env::var("SETTLE_INTERVAL_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
    let client = reqwest::Client::new();

    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let orders: Vec<serde_json::Value> = match client.get(format!("{api}/orders?status=paid")).send().await {
            Ok(r) => r.json().await.unwrap_or_default(),
            Err(e) => {
                eprintln!("bank-sim: cannot reach {api}: {e}");
                continue;
            }
        };
        for o in orders {
            let (Some(id), Some(amount)) = (o["id"].as_i64(), o["amount_cents"].as_i64()) else { continue };
            let body = serde_json::json!({
                "reference": format!("SIM-{id}"),
                "order_id": id,
                "amount_cents": amount,
            })
            .to_string();
            let res = client
                .post(format!("{api}/webhooks/bank/settlement"))
                .header("content-type", "application/json")
                .header(signature::HEADER, signature::sign(&secret, body.as_bytes()))
                .body(body)
                .send()
                .await;
            match res {
                Ok(r) => println!("bank-sim: settled order {id}: {}", r.status()),
                Err(e) => eprintln!("bank-sim: order {id}: {e}"),
            }
        }
    }
}
