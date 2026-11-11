//! Application layer: one function per use case. Each use case owns its
//! transaction boundary: load aggregates (locked), apply domain rules, persist,
//! commit. Business rules themselves live in `domain`.

pub mod error;
pub mod maintenance;
pub mod orders;
pub mod reports;
pub mod settlements;
pub mod wallets;

pub use error::{AppError, AppResult};
