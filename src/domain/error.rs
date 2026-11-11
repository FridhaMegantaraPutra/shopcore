/// A broken business rule. Infrastructure failures are not domain errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    #[error("{0}")]
    Validation(String),
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("{0}")]
    InvalidState(String),
}
