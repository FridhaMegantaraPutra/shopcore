use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::application::AppError;
use crate::domain::DomainError;

/// HTTP view of an error: a status code plus `{"error": "<message>"}`.
#[derive(Debug)]
pub enum HttpError {
    App(AppError),
    Unauthorized(&'static str),
    BadRequest(String),
}

impl From<AppError> for HttpError {
    fn from(e: AppError) -> Self {
        HttpError::App(e)
    }
}

impl From<DomainError> for HttpError {
    fn from(e: DomainError) -> Self {
        HttpError::App(AppError::Domain(e))
    }
}

pub fn status_for(e: &AppError) -> StatusCode {
    match e {
        AppError::Domain(DomainError::Validation(_)) => StatusCode::BAD_REQUEST,
        AppError::Domain(DomainError::InsufficientFunds) => StatusCode::PAYMENT_REQUIRED,
        AppError::Domain(DomainError::InvalidState(_)) => StatusCode::CONFLICT,
        AppError::NotFound(_) => StatusCode::NOT_FOUND,
        AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            HttpError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.to_string()),
            HttpError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            HttpError::App(e) => {
                let status = status_for(&e);
                if status == StatusCode::INTERNAL_SERVER_ERROR {
                    // Log the detail, never leak it to clients.
                    tracing::error!(error = %e, "internal error");
                    (status, "internal server error".to_string())
                } else {
                    (status, e.to_string())
                }
            }
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

pub type HttpResult<T> = Result<T, HttpError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_errors_to_status_codes() {
        let cases = [
            (AppError::Domain(DomainError::Validation("x".into())), StatusCode::BAD_REQUEST),
            (AppError::Domain(DomainError::InsufficientFunds), StatusCode::PAYMENT_REQUIRED),
            (AppError::Domain(DomainError::InvalidState("x".into())), StatusCode::CONFLICT),
            (AppError::NotFound("order"), StatusCode::NOT_FOUND),
            (AppError::Database(sqlx::Error::PoolTimedOut), StatusCode::INTERNAL_SERVER_ERROR),
        ];
        for (err, status) in cases {
            assert_eq!(status_for(&err), status);
        }
    }
}
