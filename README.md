# ShopCore (Rust)

ShopCore is an order, wallet and reporting service. This repository is the Rust version: an
[axum](https://github.com/tokio-rs/axum) HTTP API, a CLI for maintenance commands (the
replacement for `manage.py`), a bank settlement webhook, and a small bank simulator.

> **Note on the legacy source.** The `legacy/` repository (Flask app, `openapi.yaml`, pytest
> smoke tests) was not included in the challenge materials I received. The interface below is
> therefore my own design for the described domain (orders, wallets, reporting, bank settlement).
> I did not reverse-engineer it from the legacy code. Once the legacy spec is available, only the
> `interfaces/http` layer (routes and DTOs) and possibly the SQL column names need to change. The
> domain and application layers are independent of the wire format.

## Quick start

```sh
docker compose up --build        # Postgres + app (port 8000) + bank simulator
curl localhost:8000/health       # {"status":"ok"}
```

The app applies database migrations on startup. No manual steps are needed.

## Configuration (environment variables)

| Variable               | Required | Default        | Purpose                                      |
|------------------------|----------|----------------|----------------------------------------------|
| `DATABASE_URL`         | yes      | —              | Postgres connection string                   |
| `BANK_WEBHOOK_SECRET`  | `serve`  | —              | HMAC secret shared with the bank             |
| `BIND_ADDR`            | no       | `0.0.0.0:8000` | HTTP listen address                          |
| `RUST_LOG`             | no       | `info`         | Log filter                                   |
| `SHOPCORE_URL`         | bank-sim | `http://app:8000` | Where the simulator sends webhooks        |
| `SETTLE_INTERVAL_SECS` | bank-sim | `10`           | Simulator polling interval                   |

In compose, `BANK_WEBHOOK_SECRET` defaults to `dev-webhook-secret`. Override it for anything real.

## HTTP API

All bodies are JSON. Errors are returned as `{"error": "<message>"}`. Money is always an integer
number of cents (`*_cents`), never a float.

| Method | Path                          | Body / query                          | Success |
|--------|-------------------------------|---------------------------------------|---------|
| GET    | `/health`                     |                                       | 200     |
| POST   | `/wallets`                    | `{owner, currency?="IDR"}`            | 201     |
| GET    | `/wallets/{id}`               |                                       | 200     |
| POST   | `/wallets/{id}/topup`         | `{amount_cents}`                      | 200     |
| GET    | `/wallets/{id}/transactions`  |                                       | 200     |
| POST   | `/orders`                     | `{wallet_id, amount_cents}`           | 201     |
| GET    | `/orders`                     | `?wallet_id=&status=`                 | 200     |
| GET    | `/orders/{id}`                |                                       | 200     |
| POST   | `/orders/{id}/cancel`         |                                       | 200     |
| GET    | `/reports/daily`              | `?date=YYYY-MM-DD` (default today, UTC) | 200   |
| POST   | `/webhooks/bank/settlement`   | `{reference, order_id, amount_cents}` | 200     |

Error status codes: `400` validation, `401` bad or missing webhook signature, `402` insufficient
funds, `404` not found, `409` invalid state transition (for example cancelling a settled order) or
a settlement amount that does not match the order.

**Order lifecycle:** `paid` → `cancelled` (the wallet is refunded) or `paid` → `settled` (by the bank).

**Webhook:** the header `X-Bank-Signature: sha256=<hex HMAC-SHA256(secret, raw body)>` is
verified in constant time before the body is parsed. Each `reference` is processed at most once.
A redelivery returns `200 {"status":"duplicate"}` and changes nothing.

## Maintenance commands (`manage.py` equivalent)

```sh
docker compose exec app shopcore migrate              # apply migrations
docker compose exec app shopcore seed                 # demo wallets
docker compose exec app shopcore recompute-balances   # rebuild balances from the ledger
docker compose exec app shopcore report --date 2026-11-11
```

## Architecture (DDD)

```
src/
├── domain/          pure business rules: no DB, no HTTP, no async
│   ├── money.rs       Amount (> 0), Currency value objects
│   ├── wallet.rs      Wallet aggregate (credit/debit, no negative balance), LedgerEntry
│   ├── order.rs       Order aggregate and state machine (cancel / settle)
│   ├── settlement.rs  Settlement notification
│   └── report.rs
├── application/     one function per use case; owns the transaction boundary
├── infrastructure/  Postgres repositories, migrations, HMAC signatures
├── interfaces/http/ routes, DTOs (wire format), error → status mapping
├── main.rs          CLI (serve + maintenance commands)
└── bin/bank_sim.rs  bank simulator
```

Dependencies point inward: `interfaces → application → domain`. `application` calls
`infrastructure` repositories, which take `&mut PgConnection`, so each use case decides what runs
in one transaction.

I chose plain repository functions over repository traits. There is a single storage backend, and
the domain rules are unit-tested without a database. Traits can be introduced later if a second
backend or mocks are needed.

## Consistency guarantees

- **No overdraw under concurrency.** Wallet rows are locked with `SELECT … FOR UPDATE` before a
  debit. The domain rejects a debit when the balance is too low, and a DB `CHECK (balance_cents >= 0)`
  backs this up. This is covered by `concurrent_orders_cannot_overdraw` (10 parallel orders).
- **Ledger.** Every balance change writes a `wallet_transactions` row in the same transaction,
  so the balance always equals the sum of the ledger. `recompute-balances` repairs drift.
- **Idempotent webhook.** `settlements.reference` is `UNIQUE`, and the insert uses
  `ON CONFLICT DO NOTHING`. A rejected settlement (wrong amount or state) is rolled back, so
  its reference is not consumed.
- **No double refund.** Cancelling is only allowed from `paid`, and the order row is locked first.

## Database schema

`migrations/0001_init.sql` creates the tables `wallets`, `orders`, `wallet_transactions` and
`settlements`. The migrations are embedded in the binary (`sqlx::migrate!`), so no database or
network is needed at build time.

## Tests

```sh
cargo test                                   # unit tests (domain, signature, error mapping)

# plus end-to-end HTTP tests against a real Postgres:
docker run -d --rm -p 55432:5432 -e POSTGRES_PASSWORD=test -e POSTGRES_DB=shopcore postgres:16-alpine
DATABASE_URL=postgres://postgres:test@localhost:55432/shopcore cargo test
```

Without `DATABASE_URL`, the integration tests in `tests/api.rs` print a notice and are skipped.

### Manual test run (curl)

These are the real outputs from a fresh stack (`docker compose down -v && docker compose up -d`)
on 2026-11-11. HTTP status codes are noted in the comments.

```console
# health check → 200
$ curl -s localhost:8000/health
{"status":"ok"}

# open a wallet → 201
$ curl -s -X POST localhost:8000/wallets -H 'content-type: application/json' -d '{"owner":"fridha"}'
{"id":1,"owner":"fridha","currency":"IDR","balance_cents":0,"created_at":"2026-11-11T06:52:54.568007Z"}

# top up → 200
$ curl -s -X POST localhost:8000/wallets/1/topup -H 'content-type: application/json' -d '{"amount_cents":10000}'
{"id":1,"owner":"fridha","currency":"IDR","balance_cents":10000,"created_at":"2026-11-11T06:52:54.568007Z"}

# place an order → 201, the wallet is debited
$ curl -s -X POST localhost:8000/orders -H 'content-type: application/json' -d '{"wallet_id":1,"amount_cents":2500}'
{"id":1,"wallet_id":1,"amount_cents":2500,"status":"paid","created_at":"2026-11-11T06:52:54.648144Z"}

$ curl -s localhost:8000/wallets/1
{"id":1,"owner":"fridha","currency":"IDR","balance_cents":7500,"created_at":"2026-11-11T06:52:54.568007Z"}

# order larger than the balance → 402
$ curl -s -X POST localhost:8000/orders -H 'content-type: application/json' -d '{"wallet_id":1,"amount_cents":999999}'
{"error":"insufficient funds"}

# place a second order and cancel it → 200, the wallet is refunded
$ curl -s -X POST localhost:8000/orders -H 'content-type: application/json' -d '{"wallet_id":1,"amount_cents":1000}'
{"id":2,"wallet_id":1,"amount_cents":1000,"status":"paid","created_at":"2026-11-11T06:52:54.760947Z"}

$ curl -s -X POST localhost:8000/orders/2/cancel
{"id":2,"wallet_id":1,"amount_cents":1000,"status":"cancelled","created_at":"2026-11-11T06:52:54.760947Z"}

# cancel the same order again → 409, no second refund
$ curl -s -X POST localhost:8000/orders/2/cancel
{"error":"order is cancelled, only paid orders can be cancelled"}

# unknown wallet → 404
$ curl -s localhost:8000/wallets/999
{"error":"wallet not found"}

# webhook without a signature → 401
$ curl -s -X POST localhost:8000/webhooks/bank/settlement -H 'content-type: application/json' -d '{"reference":"FAKE","order_id":1,"amount_cents":2500}'
{"error":"missing signature"}

# about 10 s later the bank simulator has sent a signed settlement for order 1
$ curl -s localhost:8000/orders/1
{"id":1,"wallet_id":1,"amount_cents":2500,"status":"settled","created_at":"2026-11-11T06:52:54.648144Z"}

# ledger: topup, order, order, refund (the sum equals the balance of 7500)
$ curl -s localhost:8000/wallets/1/transactions
[{"id":1,"wallet_id":1,"amount_cents":10000,"kind":"topup","order_id":null,"created_at":"2026-11-11T06:52:54.602081Z"},{"id":2,"wallet_id":1,"amount_cents":-2500,"kind":"order","order_id":1,"created_at":"2026-11-11T06:52:54.648144Z"},{"id":3,"wallet_id":1,"amount_cents":-1000,"kind":"order","order_id":2,"created_at":"2026-11-11T06:52:54.760947Z"},{"id":4,"wallet_id":1,"amount_cents":1000,"kind":"refund","order_id":2,"created_at":"2026-11-11T06:52:54.802133Z"}]

$ curl -s localhost:8000/reports/daily
{"date":"2026-11-11","order_count":2,"paid_cents":0,"cancelled_cents":1000,"settled_cents":2500}

# maintenance command: the ledger and the balances agree
$ docker compose exec app shopcore recompute-balances
recomputed balances, 0 wallet(s) corrected
```

## Deliberate behaviour decisions

Without the legacy code I could not identify specific legacy bugs. These are the choices I made
on purpose:

- Money is stored and transported as integer cents, never floats.
- Validation errors return `400`. Malformed JSON or wrong field types return axum's `422`/`400`.
- Report days are UTC calendar days.
- Internal error details are logged but never returned to clients.
