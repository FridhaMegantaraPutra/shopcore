//! Postgres repositories. Every function takes `&mut PgConnection`, so the
//! application layer decides the transaction boundary (pass `&mut *tx`).

pub mod orders;
pub mod reports;
pub mod settlements;
pub mod wallets;

/// Converts a domain rule violation found while loading a row into a decode
/// error (it means the stored data is corrupt).
pub(crate) fn corrupt(e: crate::domain::DomainError) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(e))
}
