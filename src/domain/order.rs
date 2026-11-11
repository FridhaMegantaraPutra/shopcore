//! Order aggregate and its state machine:
//!
//! ```text
//! paid ──cancel──▶ cancelled
//!  │
//!  └──settle────▶ settled
//! ```

use chrono::{DateTime, Utc};

use super::money::Amount;
use super::wallet::WalletId;
use super::DomainError;

pub type OrderId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Paid,
    Cancelled,
    Settled,
}

impl OrderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            OrderStatus::Paid => "paid",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Settled => "settled",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "paid" => Ok(OrderStatus::Paid),
            "cancelled" => Ok(OrderStatus::Cancelled),
            "settled" => Ok(OrderStatus::Settled),
            other => Err(DomainError::Validation(format!("unknown order status: {other}"))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: OrderId,
    pub wallet_id: WalletId,
    pub amount: Amount,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
}

impl Order {
    /// Only paid orders can be cancelled (the caller refunds the wallet).
    pub fn cancel(&mut self) -> Result<(), DomainError> {
        self.require_paid("cancelled")?;
        self.status = OrderStatus::Cancelled;
        Ok(())
    }

    /// Only paid orders can be settled, and only for the exact amount.
    pub fn settle(&mut self, amount_cents: i64) -> Result<(), DomainError> {
        self.require_paid("settled")?;
        if amount_cents != self.amount.cents() {
            return Err(DomainError::InvalidState("amount does not match order".into()));
        }
        self.status = OrderStatus::Settled;
        Ok(())
    }

    fn require_paid(&self, action: &str) -> Result<(), DomainError> {
        if self.status != OrderStatus::Paid {
            return Err(DomainError::InvalidState(format!(
                "order is {}, only paid orders can be {action}",
                self.status.as_str()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(status: OrderStatus) -> Order {
        Order { id: 1, wallet_id: 1, amount: Amount::new(500).unwrap(), status, created_at: Utc::now() }
    }

    #[test]
    fn cancel_only_from_paid() {
        let mut o = order(OrderStatus::Paid);
        o.cancel().unwrap();
        assert_eq!(o.status, OrderStatus::Cancelled);
        assert!(o.cancel().is_err());
        assert!(order(OrderStatus::Settled).cancel().is_err());
    }

    #[test]
    fn settle_requires_paid_and_exact_amount() {
        let mut o = order(OrderStatus::Paid);
        assert!(o.settle(499).is_err());
        assert_eq!(o.status, OrderStatus::Paid);
        o.settle(500).unwrap();
        assert_eq!(o.status, OrderStatus::Settled);
        assert!(order(OrderStatus::Cancelled).settle(500).is_err());
    }

    #[test]
    fn status_roundtrip() {
        for s in [OrderStatus::Paid, OrderStatus::Cancelled, OrderStatus::Settled] {
            assert_eq!(OrderStatus::parse(s.as_str()).unwrap(), s);
        }
        assert!(OrderStatus::parse("shipped").is_err());
    }
}
