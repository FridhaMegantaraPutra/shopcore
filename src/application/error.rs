use crate::domain::DomainError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub type AppResult<T> = Result<T, AppError>;
