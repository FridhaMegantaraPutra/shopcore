//! Wallet aggregate and its append-only ledger.

use chrono::{DateTime, Utc};

use super::money::{Amount, Currency};
use super::DomainError;

pub type WalletId = i64;

/// Non-empty owner name, at most 200 characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner(String);

impl Owner {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let s = raw.trim();
        if s.is_empty() || s.chars().count() > 200 {
            return Err(DomainError::Validation("owner must be 1-200 characters".into()));
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Data needed to open a wallet (no id yet).
pub struct NewWallet {
    pub owner: Owner,
    pub currency: Currency,
}

#[derive(Debug, Clone)]
pub struct Wallet {
    pub id: WalletId,
    pub owner: String,
    pub currency: String,
    pub balance_cents: i64,
    pub created_at: DateTime<Utc>,
}

impl Wallet {
    pub fn credit(&mut self, amount: Amount) {
        self.balance_cents += amount.cents();
    }

    /// Invariant: the balance never goes negative.
    pub fn debit(&mut self, amount: Amount) -> Result<(), DomainError> {
        if self.balance_cents < amount.cents() {
            return Err(DomainError::InsufficientFunds);
        }
        self.balance_cents -= amount.cents();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Topup,
    Order,
    Refund,
}

impl EntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntryKind::Topup => "topup",
            EntryKind::Order => "order",
            EntryKind::Refund => "refund",
        }
    }
}

/// One ledger line. Invariant: wallet balance == sum of its entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub wallet_id: WalletId,
    /// Signed: positive credits the wallet, negative debits it.
    pub amount_cents: i64,
    pub kind: EntryKind,
    pub order_id: Option<i64>,
}

impl LedgerEntry {
    pub fn topup(wallet_id: WalletId, amount: Amount) -> Self {
        Self { wallet_id, amount_cents: amount.cents(), kind: EntryKind::Topup, order_id: None }
    }

    pub fn order_payment(wallet_id: WalletId, amount: Amount, order_id: i64) -> Self {
        Self { wallet_id, amount_cents: -amount.cents(), kind: EntryKind::Order, order_id: Some(order_id) }
    }

    pub fn refund(wallet_id: WalletId, amount: Amount, order_id: i64) -> Self {
        Self { wallet_id, amount_cents: amount.cents(), kind: EntryKind::Refund, order_id: Some(order_id) }
    }
}

/// A ledger entry as stored, with its id and timestamp.
#[derive(Debug, Clone)]
pub struct RecordedEntry {
    pub id: i64,
    pub entry: LedgerEntry,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wallet(balance: i64) -> Wallet {
        Wallet { id: 1, owner: "a".into(), currency: "IDR".into(), balance_cents: balance, created_at: Utc::now() }
    }

    #[test]
    fn owner_validation() {
        assert_eq!(Owner::parse("  alice ").unwrap().as_str(), "alice");
        assert!(Owner::parse("  ").is_err());
        assert!(Owner::parse(&"x".repeat(201)).is_err());
    }

    #[test]
    fn debit_respects_balance() {
        let mut w = wallet(100);
        w.debit(Amount::new(100).unwrap()).unwrap();
        assert_eq!(w.balance_cents, 0);
        assert_eq!(w.debit(Amount::new(1).unwrap()), Err(DomainError::InsufficientFunds));
        assert_eq!(w.balance_cents, 0, "failed debit must not change balance");
    }

    #[test]
    fn ledger_signs() {
        let a = Amount::new(50).unwrap();
        assert_eq!(LedgerEntry::topup(1, a).amount_cents, 50);
        assert_eq!(LedgerEntry::order_payment(1, a, 9).amount_cents, -50);
        assert_eq!(LedgerEntry::refund(1, a, 9).amount_cents, 50);
    }
}
