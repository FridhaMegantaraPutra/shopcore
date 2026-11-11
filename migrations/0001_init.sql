-- Initial schema. Amounts are stored as integer minor units (cents) to avoid
-- floating point rounding errors.
CREATE TABLE IF NOT EXISTS wallets (
    id            BIGSERIAL PRIMARY KEY,
    owner         TEXT        NOT NULL,
    currency      CHAR(3)     NOT NULL DEFAULT 'IDR',
    balance_cents BIGINT      NOT NULL DEFAULT 0 CHECK (balance_cents >= 0),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS orders (
    id           BIGSERIAL PRIMARY KEY,
    wallet_id    BIGINT      NOT NULL REFERENCES wallets(id),
    amount_cents BIGINT      NOT NULL CHECK (amount_cents > 0),
    status       TEXT        NOT NULL DEFAULT 'paid'
                 CHECK (status IN ('paid', 'cancelled', 'settled')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS orders_wallet_id_idx ON orders(wallet_id);
CREATE INDEX IF NOT EXISTS orders_created_at_idx ON orders(created_at);

-- Append-only ledger; wallet balance must always equal SUM(amount_cents).
CREATE TABLE IF NOT EXISTS wallet_transactions (
    id           BIGSERIAL PRIMARY KEY,
    wallet_id    BIGINT      NOT NULL REFERENCES wallets(id),
    amount_cents BIGINT      NOT NULL,
    kind         TEXT        NOT NULL CHECK (kind IN ('topup', 'order', 'refund')),
    order_id     BIGINT      REFERENCES orders(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS wallet_transactions_wallet_id_idx ON wallet_transactions(wallet_id);

-- Bank settlement notifications. `reference` is unique so webhook
-- redeliveries are idempotent.
CREATE TABLE IF NOT EXISTS settlements (
    id           BIGSERIAL PRIMARY KEY,
    reference    TEXT        NOT NULL UNIQUE,
    order_id     BIGINT      NOT NULL REFERENCES orders(id),
    amount_cents BIGINT      NOT NULL,
    received_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
