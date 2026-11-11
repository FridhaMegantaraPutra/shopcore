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

## Deliberate behaviour decisions

Without the legacy code I could not identify specific legacy bugs. These are the choices I made
on purpose:

- Money is stored and transported as integer cents, never floats.
- Validation errors return `400`. Malformed JSON or wrong field types return axum's `422`/`400`.
- Report days are UTC calendar days.
- Internal error details are logged but never returned to clients.
