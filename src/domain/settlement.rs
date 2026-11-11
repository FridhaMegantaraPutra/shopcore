//! Bank settlement notification. `reference` identifies a delivery; the same
//! reference arriving twice is a redelivery and must be a no-op.

use serde::Deserialize;

use super::order::OrderId;
use super::DomainError;

#[derive(Debug, Clone, Deserialize)]
pub struct Settlement {
    pub reference: String,
    pub order_id: OrderId,
    pub amount_cents: i64,
}

impl Settlement {
    pub fn validate(self) -> Result<Self, DomainError> {
        if self.reference.trim().is_empty() {
            return Err(DomainError::Validation("reference is required".into()));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementOutcome {
    Settled,
    /// Already processed this reference; nothing changed.
    Duplicate,
}
